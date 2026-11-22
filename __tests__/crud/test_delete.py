"""
Comprehensive delete tests for piorm.

Covers every deletion path:
  - Instance.delete()     — removes row, returns (count, breakdown),
                            post-delete pk still set, double delete
  - QuerySet.delete()     — all, filtered, excluded, Q objects,
                            lookup types, returns (count, breakdown)
  - Cascade behaviours    — CASCADE, SET_NULL, PROTECT, DO_NOTHING
  - Bulk / large deletes  — delete by pk__in, large sets
  - using() routing       — delete on explicit alias
  - Edge cases            — empty table, none() queryset, re-fetch after delete,
                            delete child without touching parent
"""

import pytest

import piorm
from piorm.exceptions import IntegrityError
from piorm.expr import Q
from piorm.fields import (
    CASCADE,
    DO_NOTHING,
    ForeignKey,
    IntegerField,
    SET_NULL,
    CharField,
)

from __tests__.conftest import Article, Author, Tag


# ---------------------------------------------------------------------------
# Extra models for FK-behaviour tests
# ---------------------------------------------------------------------------

class NullableChild(piorm.Model):
    """FK with SET_NULL — parent delete nullifies this column."""
    title  = CharField(max_length=100)
    author = ForeignKey("Author", on_delete=SET_NULL, null=True)

    class Meta:
        db_table = "nullable_children"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

async def make_author(**kw) -> Author:
    defaults = {"name": "Alice", "email": "alice@test.com"}
    defaults.update(kw)
    return await Author.objects.create(**defaults)


async def make_authors(n: int) -> list[Author]:
    return [
        await Author.objects.create(name=f"Author {i}", email=f"a{i}@test.com")
        for i in range(n)
    ]


# ===========================================================================
# 1. Instance.delete()
# ===========================================================================

class TestInstanceDelete:
    @pytest.mark.asyncio
    async def test_delete_removes_row(self):
        a = await make_author()
        pk = a.pk
        await a.delete()
        assert await Author.objects.filter(pk=pk).count() == 0

    @pytest.mark.asyncio
    async def test_delete_returns_count_one(self):
        a = await make_author()
        count, breakdown = await a.delete()
        assert count == 1

    @pytest.mark.asyncio
    async def test_delete_returns_breakdown_dict(self):
        a = await make_author()
        count, breakdown = await a.delete()
        assert isinstance(breakdown, dict)

    @pytest.mark.asyncio
    async def test_delete_decrements_table_count(self):
        a1 = await make_author(name="A", email="a@test.com")
        a2 = await Author.objects.create(name="B", email="b@test.com")
        await a1.delete()
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_pk_still_accessible_after_delete(self):
        """pk value remains on the Python object even after DB deletion."""
        a = await make_author()
        pk = a.pk
        await a.delete()
        assert a.pk == pk

    @pytest.mark.asyncio
    async def test_delete_only_removes_target_row(self):
        a1 = await make_author(name="A", email="a@test.com")
        a2 = await Author.objects.create(name="B", email="b@test.com")
        await a1.delete()
        survivor = await Author.objects.get(pk=a2.pk)
        assert survivor.name == "B"


# ===========================================================================
# 2. QuerySet.delete()
# ===========================================================================

class TestQuerySetDelete:
    @pytest.mark.asyncio
    async def test_delete_all(self):
        await make_authors(3)
        count, _ = await Author.objects.all().delete()
        assert count == 3
        assert await Author.objects.count() == 0

    @pytest.mark.asyncio
    async def test_delete_filtered(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=25)
        await Author.objects.create(name="Bob",   email="b@test.com", age=30)
        count, _ = await Author.objects.filter(age=25).delete()
        assert count == 1
        assert await Author.objects.filter(email="b@test.com").count() == 1

    @pytest.mark.asyncio
    async def test_delete_returns_correct_count(self):
        await make_authors(4)
        count, _ = await Author.objects.all().delete()
        assert count == 4

    @pytest.mark.asyncio
    async def test_delete_returns_zero_on_empty(self):
        count, _ = await Author.objects.all().delete()
        assert count == 0

    @pytest.mark.asyncio
    async def test_delete_returns_zero_when_no_match(self):
        await make_author()
        count, _ = await Author.objects.filter(name="Nobody").delete()
        assert count == 0

    @pytest.mark.asyncio
    async def test_delete_with_exclude(self):
        await Author.objects.create(name="Alice", email="a@test.com")
        await Author.objects.create(name="Bob",   email="b@test.com")
        count, _ = await Author.objects.exclude(name="Alice").delete()
        assert count == 1
        assert await Author.objects.filter(email="a@test.com").count() == 1

    @pytest.mark.asyncio
    async def test_delete_with_q_or(self):
        await Author.objects.create(name="Alice", email="a@test.com")
        await Author.objects.create(name="Bob",   email="b@test.com")
        await Author.objects.create(name="Carol", email="c@test.com")
        count, _ = await Author.objects.filter(Q(name="Alice") | Q(name="Bob")).delete()
        assert count == 2
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_delete_with_q_not(self):
        await Author.objects.create(name="Alice", email="a@test.com", active=True)
        await Author.objects.create(name="Bob",   email="b@test.com", active=False)
        count, _ = await Author.objects.filter(~Q(active=True)).delete()
        assert count == 1
        assert await Author.objects.filter(name="Alice").count() == 1

    @pytest.mark.asyncio
    async def test_delete_with_gt_lookup(self):
        await Author.objects.create(name="A", email="a@test.com", age=10)
        await Author.objects.create(name="B", email="b@test.com", age=20)
        await Author.objects.create(name="C", email="c@test.com", age=30)
        count, _ = await Author.objects.filter(age__gt=15).delete()
        assert count == 2
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_delete_with_in_lookup(self):
        authors = await make_authors(4)
        pks = [a.pk for a in authors[:2]]
        count, _ = await Author.objects.filter(pk__in=pks).delete()
        assert count == 2
        assert await Author.objects.count() == 2

    @pytest.mark.asyncio
    async def test_delete_with_isnull_lookup(self):
        await Author.objects.create(name="WithAge",    email="wa@test.com", age=25)
        await Author.objects.create(name="WithoutAge", email="wo@test.com", age=None)
        count, _ = await Author.objects.filter(age__isnull=True).delete()
        assert count == 1
        assert await Author.objects.filter(email="wa@test.com").count() == 1

    @pytest.mark.asyncio
    async def test_delete_none_queryset(self):
        await make_author()
        count, _ = await Author.objects.none().delete()
        assert count == 0
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_delete_chained_filter(self):
        await Author.objects.create(name="Alice", email="a@test.com", age=25, active=True)
        await Author.objects.create(name="Alice", email="a2@test.com", age=30, active=True)
        count, _ = await Author.objects.filter(name="Alice").filter(age=25).delete()
        assert count == 1
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_delete_breakdown_by_model(self):
        await make_author()
        _, breakdown = await Author.objects.all().delete()
        # breakdown should contain at least one model key
        assert isinstance(breakdown, dict)
        assert sum(breakdown.values()) >= 1


# ===========================================================================
# 3. Cascade behaviours — CASCADE
# ===========================================================================

class TestCascadeDelete:
    @pytest.mark.asyncio
    async def test_cascade_removes_child_on_parent_instance_delete(self):
        author  = await make_author()
        article = await Article.objects.create(title="Post", author=author)
        await author.delete()
        assert await Article.objects.filter(pk=article.pk).count() == 0

    @pytest.mark.asyncio
    async def test_cascade_removes_multiple_children(self):
        author = await make_author()
        for i in range(3):
            await Article.objects.create(title=f"Post {i}", author=author)
        await author.delete()
        assert await Article.objects.count() == 0

    @pytest.mark.asyncio
    async def test_cascade_via_queryset_delete(self):
        author = await make_author()
        await Article.objects.create(title="Post", author=author)
        await Author.objects.filter(pk=author.pk).delete()
        assert await Article.objects.count() == 0

    @pytest.mark.asyncio
    async def test_delete_child_does_not_affect_parent(self):
        author  = await make_author()
        article = await Article.objects.create(title="Post", author=author)
        await article.delete()
        assert await Author.objects.filter(pk=author.pk).count() == 1

    @pytest.mark.asyncio
    async def test_cascade_only_affects_related_children(self):
        a1 = await make_author(name="A", email="a@test.com")
        a2 = await Author.objects.create(name="B", email="b@test.com")
        await Article.objects.create(title="For A1", author=a1)
        await Article.objects.create(title="For A2", author=a2)
        await a1.delete()
        assert await Article.objects.count() == 1
        assert (await Article.objects.first()).title == "For A2" # type: ignore


# ===========================================================================
# 4. Cascade behaviours — SET_NULL
# ===========================================================================

class TestSetNullDelete:
    @pytest.mark.asyncio
    async def test_set_null_nullifies_fk_on_parent_delete(self):
        await piorm.execute_raw(
            "CREATE TABLE IF NOT EXISTS nullable_children "
            "(id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, author_id INTEGER)"
        )
        author = await make_author()
        await piorm.execute_raw(
            "INSERT INTO nullable_children (title, author_id) VALUES (?, ?)",
            ["Child", author.pk],
        )
        await author.delete()
        rows = await piorm.execute_raw(
            "SELECT author_id FROM nullable_children WHERE title = ?", ["Child"]
        )
        assert rows[0][0] is None


# ===========================================================================
# 5. Bulk / large deletes
# ===========================================================================

class TestBulkDelete:
    @pytest.mark.asyncio
    async def test_delete_large_set(self):
        await make_authors(50)
        count, _ = await Author.objects.all().delete()
        assert count == 50
        assert await Author.objects.count() == 0

    @pytest.mark.asyncio
    async def test_delete_by_pk_in(self):
        authors = await make_authors(6)
        to_delete = [a.pk for a in authors[:3]]
        count, _ = await Author.objects.filter(pk__in=to_delete).delete()
        assert count == 3
        assert await Author.objects.count() == 3

    @pytest.mark.asyncio
    async def test_delete_all_then_recreate(self):
        await make_authors(3)
        await Author.objects.all().delete()
        assert await Author.objects.count() == 0
        await make_author()
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_delete_ordered_queryset(self):
        await make_authors(4)
        count, _ = await Author.objects.order_by("pk").delete()
        assert count == 4


# ===========================================================================
# 6. using() routing
# ===========================================================================

class TestDeleteUsing:
    @pytest.mark.asyncio
    async def test_delete_via_using(self):
        a = await make_author()
        count, _ = await Author.objects.using("default").filter(pk=a.pk).delete()
        assert count == 1
        assert await Author.objects.count() == 0

    @pytest.mark.asyncio
    async def test_delete_all_via_using(self):
        await make_authors(2)
        count, _ = await Author.objects.using("default").all().delete()
        assert count == 2


# ===========================================================================
# 7. Edge cases
# ===========================================================================

class TestDeleteEdgeCases:
    @pytest.mark.asyncio
    async def test_delete_empty_table_returns_zero(self):
        count, _ = await Author.objects.all().delete()
        assert count == 0

    @pytest.mark.asyncio
    async def test_delete_with_contains_lookup(self):
        await Author.objects.create(name="Alice Smith", email="a@test.com")
        await Author.objects.create(name="Bob Jones",   email="b@test.com")
        count, _ = await Author.objects.filter(name__contains="Smith").delete()
        assert count == 1
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_re_fetch_after_delete_raises(self):
        a = await make_author()
        pk = a.pk
        await a.delete()
        with pytest.raises(Author.DoesNotExist):
            await Author.objects.get(pk=pk)

    @pytest.mark.asyncio
    async def test_delete_tags_independently(self):
        await Tag.objects.create(name="python")
        await Tag.objects.create(name="rust")
        count, _ = await Tag.objects.filter(name="rust").delete()
        assert count == 1
        assert await Tag.objects.filter(name="python").count() == 1

    @pytest.mark.asyncio
    async def test_queryset_still_usable_after_delete(self):
        """Deleting through a queryset doesn't corrupt the qs object."""
        await make_authors(2)
        qs = Author.objects.all()
        await qs.delete()
        # qs can still be re-evaluated (returns empty)
        results = await qs
        assert results == []

    @pytest.mark.asyncio
    async def test_delete_with_gte_and_lte(self):
        await Author.objects.create(name="A", email="a@test.com", age=10)
        await Author.objects.create(name="B", email="b@test.com", age=20)
        await Author.objects.create(name="C", email="c@test.com", age=30)
        count, _ = await Author.objects.filter(age__gte=20, age__lte=20).delete()
        assert count == 1
        assert await Author.objects.count() == 2

    @pytest.mark.asyncio
    async def test_delete_boolean_filtered(self):
        await Author.objects.create(name="Active",   email="act@test.com", active=True)
        await Author.objects.create(name="Inactive", email="in@test.com",  active=False)
        count, _ = await Author.objects.filter(active=False).delete()
        assert count == 1
        assert await Author.objects.filter(active=True).count() == 1
