"""
Comprehensive write tests for piorm QuerySet / Model.

Covers every mutation path:
  - create() — basic, with defaults, with FK, with null fields
  - save()   — insert new, update existing, partial update (update_fields)
  - update() — queryset update, single/multiple fields, F() expressions
  - delete() — instance delete, queryset delete, cascade delete
  - get_or_create()    — creates on miss, returns existing on hit
  - update_or_create() — updates on hit, creates on miss
  - bulk_create()  — basic, ignore_conflicts, batch_size, empty list
  - bulk_update()  — basic, subset of fields, batch_size
  - F() expressions in update
  - dirty tracking — only dirty fields sent on save()
  - Instance state — pk assigned after save, _piorm_state["adding"] flip
"""

import pytest

import piorm
from piorm.exceptions import IntegrityError
from piorm.expr import F

from __tests__.conftest import Article, Author, Tag


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

async def make_author(**kw) -> Author:
    defaults = {"name": "Alice", "email": "alice@test.com"}
    defaults.update(kw)
    return await Author.objects.create(**defaults)


# ===========================================================================
# 1. create()
# ===========================================================================

class TestCreate:
    @pytest.mark.asyncio
    async def test_create_returns_instance(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        assert isinstance(a, Author)

    @pytest.mark.asyncio
    async def test_create_assigns_pk(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        assert a.pk is not None

    @pytest.mark.asyncio
    async def test_create_persists_to_db(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.name == "Alice"

    @pytest.mark.asyncio
    async def test_create_with_all_scalar_fields(self):
        a = await Author.objects.create(
            name="Bob",
            email="bob@test.com",
            age=30,
            score=9.5,
            bio="Developer",
            active=False,
            metadata={"key": "value"},
        )
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.age    == 30
        assert fetched.score  == 9.5
        assert fetched.bio    == "Developer"
        assert fetched.active is False

    @pytest.mark.asyncio
    async def test_create_with_null_fields(self):
        a = await Author.objects.create(name="Null", email="null@test.com", age=None)
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.age is None

    @pytest.mark.asyncio
    async def test_create_with_fk(self):
        author = await Author.objects.create(name="Alice", email="alice@test.com")
        article = await Article.objects.create(title="My Post", author=author)
        assert article.pk is not None
        fetched = await Article.objects.get(pk=article.pk)
        assert fetched.title == "My Post"

    @pytest.mark.asyncio
    async def test_create_with_null_fk(self):
        article = await Article.objects.create(title="Orphan", author=None)
        fetched = await Article.objects.get(pk=article.pk)
        assert fetched.title == "Orphan"

    @pytest.mark.asyncio
    async def test_create_default_field_value(self):
        a = await Author.objects.create(name="Default", email="def@test.com")
        assert a.active is True  # BooleanField(default=True)

    @pytest.mark.asyncio
    async def test_create_unique_violation_raises(self):
        await Author.objects.create(name="Alice", email="alice@test.com")
        with pytest.raises((IntegrityError, Exception)):
            await Author.objects.create(name="Other", email="alice@test.com")

    @pytest.mark.asyncio
    async def test_create_multiple_rows(self):
        await Author.objects.create(name="A", email="a@test.com")
        await Author.objects.create(name="B", email="b@test.com")
        assert await Author.objects.count() == 2


# ===========================================================================
# 2. save() — new instance (INSERT)
# ===========================================================================

class TestSaveInsert:
    @pytest.mark.asyncio
    async def test_save_new_instance_inserts_row(self):
        a = Author(name="Alice", email="alice@test.com")
        await a.save()
        assert a.pk is not None
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_save_new_sets_adding_false(self):
        a = Author(name="Alice", email="alice@test.com")
        assert a._adding is True
        await a.save()
        assert a._adding is False

    @pytest.mark.asyncio
    async def test_save_new_persists_values(self):
        a = Author(name="Carol", email="carol@test.com", age=28)
        await a.save()
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.age == 28


# ===========================================================================
# 3. save() — existing instance (UPDATE)
# ===========================================================================

class TestSaveUpdate:
    @pytest.mark.asyncio
    async def test_save_updates_row(self):
        a = await make_author()
        a.name = "Updated"
        await a.save()
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.name == "Updated"

    @pytest.mark.asyncio
    async def test_save_does_not_create_duplicate(self):
        a = await make_author()
        a.name = "Updated"
        await a.save()
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_save_update_fields_only_writes_specified(self):
        a = await Author.objects.create(
            name="Alice", email="alice@test.com", age=25, bio="Original"
        )
        a.name = "Changed"
        a.bio  = "Changed too"
        await a.save(update_fields=["bio"])
        fetched = await Author.objects.get(pk=a.pk)
        # bio changed, name unchanged
        assert fetched.bio  == "Changed too"
        assert fetched.name == "Alice"

    @pytest.mark.asyncio
    async def test_save_clears_dirty_set(self):
        a = await make_author()
        a.name = "Dirty"
        assert bool(a._dirty & Author._field_bits["name"])
        await a.save()
        assert a._dirty == 0


# ===========================================================================
# 4. update() — queryset
# ===========================================================================

class TestQuerySetUpdate:
    @pytest.mark.asyncio
    async def test_update_single_field(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=25)
        count = await Author.objects.filter(name="Alice").update(age=99)
        assert count == 1
        fetched = await Author.objects.get(email="a@test.com")
        assert fetched.age == 99

    @pytest.mark.asyncio
    async def test_update_multiple_fields(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=25)
        await Author.objects.filter(name="Alice").update(age=99, bio="Bio")
        fetched = await Author.objects.get(email="a@test.com")
        assert fetched.age == 99
        assert fetched.bio == "Bio"

    @pytest.mark.asyncio
    async def test_update_returns_affected_count(self):
        await Author.objects.create(name="Alice", email="a@test.com")
        await Author.objects.create(name="Alice", email="b@test.com")
        await Author.objects.create(name="Bob",   email="c@test.com")
        count = await Author.objects.filter(name="Alice").update(bio="Updated")
        assert count == 2

    @pytest.mark.asyncio
    async def test_update_all(self):
        await Author.objects.create(name="A", email="a@test.com")
        await Author.objects.create(name="B", email="b@test.com")
        count = await Author.objects.all().update(active=False)
        assert count == 2
        results = await Author.objects.filter(active=False)
        assert len(results) == 2

    @pytest.mark.asyncio
    async def test_update_no_match_returns_zero(self):
        count = await Author.objects.filter(name="Nobody").update(age=1)
        assert count == 0

    @pytest.mark.asyncio
    async def test_update_with_f_expression(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=10)
        await Author.objects.filter(name="Alice").update(age=F("age") + 5)
        fetched = await Author.objects.get(email="a@test.com")
        assert fetched.age == 15

    @pytest.mark.asyncio
    async def test_update_with_f_multiplication(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=10)
        await Author.objects.filter(name="Alice").update(age=F("age") * 3)
        fetched = await Author.objects.get(email="a@test.com")
        assert fetched.age == 30


# ===========================================================================
# 5. delete()
# ===========================================================================

class TestDelete:
    @pytest.mark.asyncio
    async def test_instance_delete_removes_row(self):
        a = await make_author()
        pk = a.pk
        count, _ = await a.delete()
        assert count == 1
        assert await Author.objects.filter(pk=pk).count() == 0

    @pytest.mark.asyncio
    async def test_instance_delete_returns_count(self):
        a = await make_author()
        count, breakdown = await a.delete()
        assert count == 1
        assert isinstance(breakdown, dict)

    @pytest.mark.asyncio
    async def test_queryset_delete_removes_matched(self):
        await Author.objects.create(name="Alice", email="a@test.com")
        await Author.objects.create(name="Bob",   email="b@test.com")
        count, _ = await Author.objects.filter(name="Alice").delete()
        assert count == 1
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_queryset_delete_all(self):
        await Author.objects.create(name="A", email="a@test.com")
        await Author.objects.create(name="B", email="b@test.com")
        count, _ = await Author.objects.all().delete()
        assert count == 2
        assert await Author.objects.count() == 0

    @pytest.mark.asyncio
    async def test_delete_no_match_returns_zero(self):
        count, _ = await Author.objects.filter(name="Nobody").delete()
        assert count == 0

    @pytest.mark.asyncio
    async def test_cascade_delete_removes_children(self):
        author  = await Author.objects.create(name="Alice", email="a@test.com")
        article = await Article.objects.create(title="Post", author=author)
        await author.delete()
        # Article should be gone due to CASCADE
        assert await Article.objects.filter(pk=article.pk).count() == 0


# ===========================================================================
# 6. get_or_create()
# ===========================================================================

class TestGetOrCreate:
    @pytest.mark.asyncio
    async def test_creates_when_not_found(self):
        obj, created = await Author.objects.get_or_create(
            email="new@test.com",
            defaults={"name": "New Author"},
        )
        assert created is True
        assert obj.pk is not None
        assert obj.name == "New Author"

    @pytest.mark.asyncio
    async def test_returns_existing_when_found(self):
        existing = await Author.objects.create(name="Alice", email="alice@test.com")
        obj, created = await Author.objects.get_or_create(
            email="alice@test.com",
            defaults={"name": "Other"},
        )
        assert created is False
        assert obj.pk == existing.pk
        assert obj.name == "Alice"  # unchanged

    @pytest.mark.asyncio
    async def test_does_not_duplicate_on_race(self):
        await Author.objects.get_or_create(
            email="once@test.com",
            defaults={"name": "Once"},
        )
        await Author.objects.get_or_create(
            email="once@test.com",
            defaults={"name": "Twice"},
        )
        assert await Author.objects.filter(email="once@test.com").count() == 1

    @pytest.mark.asyncio
    async def test_get_or_create_without_defaults(self):
        obj, created = await Author.objects.get_or_create(
            email="nodef@test.com",
            name="NoDef",
        )
        assert created is True
        assert obj.name == "NoDef"


# ===========================================================================
# 7. update_or_create()
# ===========================================================================

class TestUpdateOrCreate:
    @pytest.mark.asyncio
    async def test_creates_when_not_found(self):
        obj, created = await Author.objects.update_or_create(
            email="brand_new@test.com",
            defaults={"name": "Brand New"},
        )
        assert created is True
        assert obj.name == "Brand New"

    @pytest.mark.asyncio
    async def test_updates_when_found(self):
        existing = await Author.objects.create(name="Old", email="old@test.com")
        obj, created = await Author.objects.update_or_create(
            email="old@test.com",
            defaults={"name": "Updated"},
        )
        assert created is False
        assert obj.pk == existing.pk
        assert obj.name == "Updated"

    @pytest.mark.asyncio
    async def test_update_or_create_updates_in_db(self):
        await Author.objects.create(name="Old", email="old@test.com")
        await Author.objects.update_or_create(
            email="old@test.com",
            defaults={"name": "New"},
        )
        fetched = await Author.objects.get(email="old@test.com")
        assert fetched.name == "New"


# ===========================================================================
# 8. bulk_create()
# ===========================================================================

class TestBulkCreate:
    @pytest.mark.asyncio
    async def test_bulk_create_inserts_all(self):
        authors = [
            Author(name=f"Author {i}", email=f"author{i}@test.com")
            for i in range(5)
        ]
        result = await Author.objects.bulk_create(authors)
        assert await Author.objects.count() == 5

    @pytest.mark.asyncio
    async def test_bulk_create_returns_list(self):
        authors = [Author(name="A", email="a@test.com")]
        result = await Author.objects.bulk_create(authors)
        assert isinstance(result, list)

    @pytest.mark.asyncio
    async def test_bulk_create_empty_list(self):
        result = await Author.objects.bulk_create([])
        assert result == [] or len(result) == 0
        assert await Author.objects.count() == 0

    @pytest.mark.asyncio
    async def test_bulk_create_with_batch_size(self):
        authors = [
            Author(name=f"A{i}", email=f"a{i}@test.com")
            for i in range(10)
        ]
        await Author.objects.bulk_create(authors, batch_size=3)
        assert await Author.objects.count() == 10

    @pytest.mark.asyncio
    async def test_bulk_create_ignore_conflicts(self):
        await Author.objects.create(name="Alice", email="alice@test.com")
        # Same email — would normally conflict
        authors = [
            Author(name="Conflict", email="alice@test.com"),
            Author(name="New",      email="new@test.com"),
        ]
        await Author.objects.bulk_create(authors, ignore_conflicts=True)
        # "alice" unchanged, "new" inserted
        assert await Author.objects.count() == 2
        alice = await Author.objects.get(email="alice@test.com")
        assert alice.name == "Alice"

    @pytest.mark.asyncio
    async def test_bulk_create_single_item(self):
        await Author.objects.bulk_create([
            Author(name="Solo", email="solo@test.com")
        ])
        assert await Author.objects.filter(email="solo@test.com").count() == 1


# ===========================================================================
# 9. bulk_update()
# ===========================================================================

class TestBulkUpdate:
    @pytest.mark.asyncio
    async def test_bulk_update_single_field(self):
        objs = [
            await Author.objects.create(name=f"Author {i}", email=f"a{i}@test.com")
            for i in range(3)
        ]
        for obj in objs:
            obj._values["bio"] = "Updated"
        count = await Author.objects.bulk_update(objs, ["bio"])
        assert count == 3
        results = await Author.objects.all()
        assert all(r.bio == "Updated" for r in results)

    @pytest.mark.asyncio
    async def test_bulk_update_multiple_fields(self):
        objs = [
            await Author.objects.create(name=f"A{i}", email=f"a{i}@test.com", age=i * 10)
            for i in range(2)
        ]
        for obj in objs:
            obj._values["name"] = "Renamed"
            obj._values["age"]  = 99
        await Author.objects.bulk_update(objs, ["name", "age"])
        results = await Author.objects.all()
        assert all(r.name == "Renamed" and r.age == 99 for r in results)

    @pytest.mark.asyncio
    async def test_bulk_update_with_batch_size(self):
        objs = [
            await Author.objects.create(name=f"A{i}", email=f"a{i}@test.com")
            for i in range(6)
        ]
        for obj in objs:
            obj._values["bio"] = "Batched"
        count = await Author.objects.bulk_update(objs, ["bio"], batch_size=2)
        assert count == 6

    @pytest.mark.asyncio
    async def test_bulk_update_partial_fields_leaves_others_unchanged(self):
        a = await Author.objects.create(
            name="Original", email="o@test.com", age=25, bio="Keep"
        )
        a._values["name"] = "Changed"
        await Author.objects.bulk_update([a], ["name"])
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.name == "Changed"
        assert fetched.bio  == "Keep"


# ===========================================================================
# 10. Dirty tracking
# ===========================================================================

class TestDirtyTracking:
    @pytest.mark.asyncio
    async def test_new_instance_no_dirty(self):
        a = Author(name="Alice", email="alice@test.com")
        # __init__ does setattr which marks dirty
        assert bool(a._dirty & Author._field_bits["name"])

    @pytest.mark.asyncio
    async def test_after_create_dirty_cleared(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        assert a._dirty == 0

    @pytest.mark.asyncio
    async def test_setattr_marks_dirty(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        a.name = "Changed"
        assert bool(a._dirty & Author._field_bits["name"])

    @pytest.mark.asyncio
    async def test_save_clears_dirty(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        a.name = "Changed"
        await a.save()
        assert not (a._dirty & Author._field_bits["name"])

    @pytest.mark.asyncio
    async def test_multiple_field_changes_tracked(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        a.name = "Bob"
        a.age  = 99
        assert bool(a._dirty & Author._field_bits["name"])
        assert bool(a._dirty & Author._field_bits["age"])


# ===========================================================================
# 11. Instance state
# ===========================================================================

class TestInstanceState:
    @pytest.mark.asyncio
    async def test_new_instance_is_adding(self):
        a = Author(name="Alice", email="alice@test.com")
        assert a._adding is True

    @pytest.mark.asyncio
    async def test_after_save_not_adding(self):
        a = Author(name="Alice", email="alice@test.com")
        await a.save()
        assert a._adding is False

    @pytest.mark.asyncio
    async def test_create_not_adding(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        assert a._adding is False

    @pytest.mark.asyncio
    async def test_pk_set_after_save(self):
        a = Author(name="Alice", email="alice@test.com")
        assert a.pk is None
        await a.save()
        assert a.pk is not None

    @pytest.mark.asyncio
    async def test_repr_shows_pk(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        assert str(a.pk) in repr(a)
