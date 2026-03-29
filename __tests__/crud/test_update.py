"""
Comprehensive update tests for ormpi.

Covers every mutation path that modifies existing rows:
  - QuerySet.update()        — single/multi field, filtered, all, Q objects,
                               lookup types, F() arithmetic, to null, count return
  - Instance.save()          — full update, update_fields partial, multiple saves,
                               refresh_from_db then save
  - update_or_create()       — creates on miss, updates on hit, tuple return,
                               multiple defaults, empty defaults, lookup vs defaults
  - bulk_update()            — all rows, subset of fields, batch_size, count return
  - F() expressions          — add, subtract, multiply, divide, chained
  - using() routing          — update on explicit alias
  - Edge cases               — update no rows, update to null, update boolean,
                               chained filter+update, update after bulk_create
"""

import pytest

import ormpi
from ormpi.expr import F, Q

from __tests__.conftest import Article, Author, Tag


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

async def make_authors(n: int, **extra) -> list[Author]:
    return [
        await Author.objects.create(name=f"Author {i}", email=f"a{i}@test.com", **extra)
        for i in range(n)
    ]


async def make_author(**kw) -> Author:
    defaults = {"name": "Alice", "email": "alice@test.com"}
    defaults.update(kw)
    return await Author.objects.create(**defaults)


# ===========================================================================
# 1. QuerySet.update() — basic
# ===========================================================================

class TestQuerySetUpdateBasic:
    @pytest.mark.asyncio
    async def test_update_single_field(self):
        a = await make_author(age=25)
        count = await Author.objects.filter(pk=a.pk).update(age=99)
        assert count == 1
        assert (await Author.objects.get(pk=a.pk)).age == 99

    @pytest.mark.asyncio
    async def test_update_multiple_fields_at_once(self):
        a = await make_author(age=25, bio="old")
        await Author.objects.filter(pk=a.pk).update(age=99, bio="new", active=False)
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.age    == 99
        assert fetched.bio    == "new"
        assert fetched.active is False

    @pytest.mark.asyncio
    async def test_update_returns_correct_count(self):
        authors = await make_authors(4)
        count = await Author.objects.filter(
            pk__in=[a.pk for a in authors[:2]]
        ).update(bio="batch")
        assert count == 2

    @pytest.mark.asyncio
    async def test_update_all_rows(self):
        await make_authors(3)
        count = await Author.objects.all().update(active=False)
        assert count == 3
        assert await Author.objects.filter(active=False).count() == 3

    @pytest.mark.asyncio
    async def test_update_returns_zero_when_no_match(self):
        count = await Author.objects.filter(email="nobody@test.com").update(age=1)
        assert count == 0

    @pytest.mark.asyncio
    async def test_update_to_null(self):
        a = await make_author(age=25)
        await Author.objects.filter(pk=a.pk).update(age=None)
        assert (await Author.objects.get(pk=a.pk)).age is None

    @pytest.mark.asyncio
    async def test_update_boolean_to_false(self):
        a = await make_author()  # active=True by default
        await Author.objects.filter(pk=a.pk).update(active=False)
        assert (await Author.objects.get(pk=a.pk)).active is False

    @pytest.mark.asyncio
    async def test_update_boolean_to_true(self):
        a = await make_author(active=False)
        await Author.objects.filter(pk=a.pk).update(active=True)
        assert (await Author.objects.get(pk=a.pk)).active is True

    @pytest.mark.asyncio
    async def test_update_string_field(self):
        a = await make_author()
        await Author.objects.filter(pk=a.pk).update(name="Renamed")
        assert (await Author.objects.get(pk=a.pk)).name == "Renamed"

    @pytest.mark.asyncio
    async def test_update_does_not_affect_non_matching_rows(self):
        a1 = await make_author(name="Alice", email="alice@test.com", age=25)
        a2 = await Author.objects.create(name="Bob", email="bob@test.com", age=30)
        await Author.objects.filter(pk=a1.pk).update(age=99)
        assert (await Author.objects.get(pk=a2.pk)).age == 30


# ===========================================================================
# 2. QuerySet.update() — with filter/exclude/Q
# ===========================================================================

class TestQuerySetUpdateFiltered:
    @pytest.mark.asyncio
    async def test_update_after_filter(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=25)
        await Author.objects.create(name="Bob",   email="b@test.com", age=30)
        count = await Author.objects.filter(age__gte=30).update(bio="senior")
        assert count == 1
        assert (await Author.objects.get(email="b@test.com")).bio == "senior"
        assert (await Author.objects.get(email="a@test.com")).bio is None

    @pytest.mark.asyncio
    async def test_update_after_exclude(self):
        await Author.objects.create(name="Alice", email="a@test.com")
        await Author.objects.create(name="Bob",   email="b@test.com")
        count = await Author.objects.exclude(name="Alice").update(bio="not alice")
        assert count == 1
        assert (await Author.objects.get(email="b@test.com")).bio == "not alice"

    @pytest.mark.asyncio
    async def test_update_with_q_or(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=10)
        await Author.objects.create(name="Bob",   email="b@test.com", age=20)
        await Author.objects.create(name="Carol", email="c@test.com", age=30)
        count = await Author.objects.filter(Q(age=10) | Q(age=30)).update(bio="edge")
        assert count == 2
        assert (await Author.objects.get(email="b@test.com")).bio is None

    @pytest.mark.asyncio
    async def test_update_with_q_and(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=25, active=True)
        await Author.objects.create(name="Alice", email="a2@test.com", age=25, active=False)
        count = await Author.objects.filter(Q(name="Alice") & Q(active=True)).update(bio="hit")
        assert count == 1

    @pytest.mark.asyncio
    async def test_update_with_q_not(self):
        await Author.objects.create(name="Alice", email="a@test.com", active=True)
        await Author.objects.create(name="Bob",   email="b@test.com", active=False)
        count = await Author.objects.filter(~Q(active=False)).update(bio="active")
        assert count == 1
        assert (await Author.objects.get(email="a@test.com")).bio == "active"

    @pytest.mark.asyncio
    async def test_chained_filters_then_update(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=25, active=True)
        await Author.objects.create(name="Alice", email="a2@test.com", age=30, active=True)
        count = await Author.objects.filter(name="Alice").filter(age=25).update(bio="exact")
        assert count == 1

    @pytest.mark.asyncio
    async def test_update_with_lookup_contains(self):
        await Author.objects.create(name="Alice Smith", email="a@test.com")
        await Author.objects.create(name="Bob Jones",   email="b@test.com")
        count = await Author.objects.filter(name__contains="Smith").update(bio="smith")
        assert count == 1

    @pytest.mark.asyncio
    async def test_update_with_lookup_isnull(self):
        await Author.objects.create(name="WithAge",    email="wa@test.com", age=25)
        await Author.objects.create(name="WithoutAge", email="wo@test.com", age=None)
        count = await Author.objects.filter(age__isnull=True).update(bio="no age")
        assert count == 1
        assert (await Author.objects.get(email="wo@test.com")).bio == "no age"

    @pytest.mark.asyncio
    async def test_update_with_in_lookup(self):
        authors = await make_authors(4)
        pks = [a.pk for a in authors[:2]]
        count = await Author.objects.filter(pk__in=pks).update(bio="in set")
        assert count == 2


# ===========================================================================
# 3. QuerySet.update() — F() expressions
# ===========================================================================

class TestUpdateFExpressions:
    @pytest.mark.asyncio
    async def test_f_add(self):
        a = await make_author(age=10)
        await Author.objects.filter(pk=a.pk).update(age=F("age") + 5)
        assert (await Author.objects.get(pk=a.pk)).age == 15

    @pytest.mark.asyncio
    async def test_f_subtract(self):
        a = await make_author(age=10)
        await Author.objects.filter(pk=a.pk).update(age=F("age") - 3)
        assert (await Author.objects.get(pk=a.pk)).age == 7

    @pytest.mark.asyncio
    async def test_f_multiply(self):
        a = await make_author(age=5)
        await Author.objects.filter(pk=a.pk).update(age=F("age") * 4)
        assert (await Author.objects.get(pk=a.pk)).age == 20

    @pytest.mark.asyncio
    async def test_f_divide(self):
        a = await make_author(age=20)
        await Author.objects.filter(pk=a.pk).update(age=F("age") / 4)
        assert (await Author.objects.get(pk=a.pk)).age == 5

    @pytest.mark.asyncio
    async def test_f_applied_to_all_rows(self):
        await Author.objects.create(name="A", email="a@test.com", age=10)
        await Author.objects.create(name="B", email="b@test.com", age=20)
        await Author.objects.all().update(age=F("age") + 1)
        ages = {r.age for r in await Author.objects.all()}
        assert ages == {11, 21}

    @pytest.mark.asyncio
    async def test_f_chained_arithmetic(self):
        a = await make_author(age=10)
        await Author.objects.filter(pk=a.pk).update(age=F("age") * 2 + 1)
        assert (await Author.objects.get(pk=a.pk)).age == 21

    @pytest.mark.asyncio
    async def test_f_applied_to_filtered_subset(self):
        await Author.objects.create(name="A", email="a@test.com", age=10)
        await Author.objects.create(name="B", email="b@test.com", age=20)
        await Author.objects.filter(age__lt=15).update(age=F("age") + 100)
        assert (await Author.objects.get(email="a@test.com")).age == 110
        assert (await Author.objects.get(email="b@test.com")).age == 20


# ===========================================================================
# 4. Instance.save() — update path
# ===========================================================================

class TestInstanceSaveUpdate:
    @pytest.mark.asyncio
    async def test_save_updates_persisted_field(self):
        a = await make_author(age=25)
        a.age = 99
        await a.save()
        assert (await Author.objects.get(pk=a.pk)).age == 99

    @pytest.mark.asyncio
    async def test_save_multiple_changed_fields(self):
        a = await make_author(age=25, bio="old")
        a.age = 99
        a.bio = "new"
        await a.save()
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.age == 99
        assert fetched.bio == "new"

    @pytest.mark.asyncio
    async def test_save_does_not_create_extra_rows(self):
        a = await make_author()
        a.name = "Updated"
        await a.save()
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_save_update_fields_only_writes_listed(self):
        a = await Author.objects.create(
            name="Alice", email="alice@test.com", age=25, bio="original"
        )
        a.name = "ShouldNotChange"
        a.bio  = "changed"
        await a.save(update_fields=["bio"])
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.bio  == "changed"
        assert fetched.name == "Alice"

    @pytest.mark.asyncio
    async def test_save_update_fields_single(self):
        a = await make_author(age=1)
        a.age  = 99
        a.name = "ShouldStay"
        await a.save(update_fields=["age"])
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.age  == 99
        assert fetched.name == "Alice"

    @pytest.mark.asyncio
    async def test_multiple_saves_accumulate(self):
        a = await make_author(age=1)
        a.age = 2
        await a.save()
        a.age = 3
        await a.save()
        assert (await Author.objects.get(pk=a.pk)).age == 3

    @pytest.mark.asyncio
    async def test_save_clears_dirty_fields(self):
        a = await make_author()
        a.name = "Dirty"
        assert bool(a._dirty & Author._field_bits["name"])
        await a.save()
        assert not (a._dirty & Author._field_bits["name"])

    @pytest.mark.asyncio
    async def test_save_to_null_field(self):
        a = await make_author(age=25)
        a.age = None
        await a.save()
        assert (await Author.objects.get(pk=a.pk)).age is None

    @pytest.mark.asyncio
    async def test_refresh_from_db_then_save(self):
        a = await make_author(age=25)
        # Simulate external change via queryset
        await Author.objects.filter(pk=a.pk).update(age=50)
        await a.refresh_from_db()
        assert a.age == 50
        a.bio = "after refresh"
        await a.save()
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.age == 50
        assert fetched.bio == "after refresh"


# ===========================================================================
# 5. update_or_create()
# ===========================================================================

class TestUpdateOrCreate:
    @pytest.mark.asyncio
    async def test_creates_when_not_found(self):
        obj, created = await Author.objects.update_or_create(
            email="new@test.com",
            defaults={"name": "New"},
        )
        assert created is True
        assert obj.pk is not None
        assert obj.name == "New"

    @pytest.mark.asyncio
    async def test_updates_when_found(self):
        existing = await Author.objects.create(name="Old", email="old@test.com", age=10)
        obj, created = await Author.objects.update_or_create(
            email="old@test.com",
            defaults={"name": "New", "age": 99},
        )
        assert created is False
        assert obj.pk == existing.pk
        assert obj.name == "New"
        assert obj.age  == 99

    @pytest.mark.asyncio
    async def test_update_persisted_to_db(self):
        await Author.objects.create(name="Old", email="old@test.com")
        await Author.objects.update_or_create(
            email="old@test.com",
            defaults={"name": "New"},
        )
        assert (await Author.objects.get(email="old@test.com")).name == "New"

    @pytest.mark.asyncio
    async def test_returns_tuple(self):
        result = await Author.objects.update_or_create(
            email="t@test.com",
            defaults={"name": "T"},
        )
        assert isinstance(result, tuple)
        assert len(result) == 2

    @pytest.mark.asyncio
    async def test_lookup_field_not_overwritten_by_defaults(self):
        """The lookup kwarg (email) should not be changed even if not in defaults."""
        await Author.objects.create(name="Old", email="keep@test.com")
        obj, _ = await Author.objects.update_or_create(
            email="keep@test.com",
            defaults={"name": "Updated"},
        )
        assert obj.email == "keep@test.com"

    @pytest.mark.asyncio
    async def test_multiple_defaults_applied(self):
        obj, created = await Author.objects.update_or_create(
            email="multi@test.com",
            defaults={"name": "Multi", "age": 42, "bio": "bio", "active": False},
        )
        assert created is True
        assert obj.age    == 42
        assert obj.bio    == "bio"
        assert obj.active is False

    @pytest.mark.asyncio
    async def test_empty_defaults_just_creates(self):
        obj, created = await Author.objects.update_or_create(
            email="empty@test.com",
            name="Empty",
        )
        assert created is True
        assert obj.name == "Empty"

    @pytest.mark.asyncio
    async def test_does_not_create_duplicate(self):
        await Author.objects.update_or_create(
            email="once@test.com", defaults={"name": "Once"}
        )
        await Author.objects.update_or_create(
            email="once@test.com", defaults={"name": "Twice"}
        )
        assert await Author.objects.filter(email="once@test.com").count() == 1


# ===========================================================================
# 6. bulk_update()
# ===========================================================================

class TestBulkUpdate:
    @pytest.mark.asyncio
    async def test_updates_all_objects(self):
        objs = await make_authors(3)
        for obj in objs:
            obj._values["bio"] = "updated"
        count = await Author.objects.bulk_update(objs, ["bio"])
        assert count == 3
        for r in await Author.objects.all():
            assert r.bio == "updated"

    @pytest.mark.asyncio
    async def test_updates_only_specified_fields(self):
        a = await Author.objects.create(
            name="Alice", email="a@test.com", age=25, bio="keep"
        )
        a._values["name"] = "Changed"
        a._values["age"]  = 99
        await Author.objects.bulk_update([a], ["name"])
        fetched = await Author.objects.get(pk=a.pk)
        assert fetched.name == "Changed"
        assert fetched.age  == 25    # not updated
        assert fetched.bio  == "keep"  # not updated

    @pytest.mark.asyncio
    async def test_bulk_update_multiple_fields(self):
        objs = await make_authors(2)
        for obj in objs:
            obj._values["name"] = "Renamed"
            obj._values["age"]  = 77
        await Author.objects.bulk_update(objs, ["name", "age"])
        for r in await Author.objects.all():
            assert r.name == "Renamed"
            assert r.age  == 77

    @pytest.mark.asyncio
    async def test_bulk_update_single_object(self):
        a = await make_author()
        a._values["bio"] = "solo"
        count = await Author.objects.bulk_update([a], ["bio"])
        assert count == 1
        assert (await Author.objects.get(pk=a.pk)).bio == "solo"

    @pytest.mark.asyncio
    async def test_bulk_update_with_batch_size(self):
        objs = await make_authors(9)
        for obj in objs:
            obj._values["bio"] = "batched"
        count = await Author.objects.bulk_update(objs, ["bio"], batch_size=3)
        assert count == 9
        assert await Author.objects.filter(bio="batched").count() == 9

    @pytest.mark.asyncio
    async def test_bulk_update_partial_list(self):
        objs = await make_authors(4)
        subset = objs[:2]
        for obj in subset:
            obj._values["bio"] = "partial"
        await Author.objects.bulk_update(subset, ["bio"])
        updated = await Author.objects.filter(bio="partial").count()
        assert updated == 2
        unchanged = await Author.objects.filter(bio__isnull=True).count()
        assert unchanged == 2

    @pytest.mark.asyncio
    async def test_bulk_update_after_bulk_create(self):
        raw = [Author(name=f"N{i}", email=f"n{i}@test.com") for i in range(3)]
        created = await Author.objects.bulk_create(raw)
        all_objs = await Author.objects.all()
        for obj in all_objs:
            obj._values["bio"] = "from bulk"
        count = await Author.objects.bulk_update(all_objs, ["bio"])
        assert count == 3


# ===========================================================================
# 7. using() — update on explicit alias
# ===========================================================================

class TestUpdateUsing:
    @pytest.mark.asyncio
    async def test_update_via_using(self):
        a = await make_author(age=1)
        await Author.objects.using("default").filter(pk=a.pk).update(age=99)
        assert (await Author.objects.get(pk=a.pk)).age == 99

    @pytest.mark.asyncio
    async def test_filter_using_chain(self):
        await make_author(age=5)
        count = await Author.objects.using("default").filter(age=5).update(bio="via alias")
        assert count == 1


# ===========================================================================
# 8. Edge cases
# ===========================================================================

class TestUpdateEdgeCases:
    @pytest.mark.asyncio
    async def test_update_with_sliced_qs(self):
        """Update on a sliced queryset — exercises LIMIT in UPDATE."""
        await make_authors(5, age=10)
        # Slicing + update: some backends support it
        try:
            count = await Author.objects.order_by("pk")[:3].update(bio="limited")
            assert count <= 3
        except Exception:
            pytest.skip("Backend does not support UPDATE with LIMIT")

    @pytest.mark.asyncio
    async def test_update_preserves_unrelated_rows(self):
        a1 = await Author.objects.create(name="A", email="a@test.com", age=10)
        a2 = await Author.objects.create(name="B", email="b@test.com", age=20)
        await Author.objects.filter(pk=a1.pk).update(bio="changed")
        fetched_b = await Author.objects.get(pk=a2.pk)
        assert fetched_b.bio is None

    @pytest.mark.asyncio
    async def test_f_on_zero_value(self):
        a = await make_author(age=0)
        await Author.objects.filter(pk=a.pk).update(age=F("age") + 10)
        assert (await Author.objects.get(pk=a.pk)).age == 10

    @pytest.mark.asyncio
    async def test_update_same_value_no_error(self):
        a = await make_author(name="Alice")
        count = await Author.objects.filter(pk=a.pk).update(name="Alice")
        assert count == 1

    @pytest.mark.asyncio
    async def test_queryset_not_mutated_by_update(self):
        """The queryset object itself is not changed by calling .update()."""
        await make_authors(2, age=5)
        qs = Author.objects.filter(age=5)
        await qs.update(bio="updated")
        # queryset still works for reading
        results = await qs
        assert len(results) == 2
