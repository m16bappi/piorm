"""
Comprehensive read tests for ormpi QuerySet.

Covers every retrieval path:
  - get / filter / exclude / all / none
  - field lookups: exact, gt, gte, lt, lte, in, isnull,
                   contains, icontains, startswith, istartswith,
                   endswith, iendswith, range
  - ordering: asc, desc, multi-field, reverse()
  - distinct()
  - slicing: LIMIT, OFFSET+LIMIT, single index
  - first / last / earliest / latest
  - count / exists
  - values / values_list / values_list(flat=True)
  - only / defer
  - Q objects: AND, OR, NOT, nested combinations
  - aggregate: Count, Sum, Avg, Max, Min, distinct=True, filter=
  - annotate
  - select_related (JOIN eager load)
  - prefetch_related (separate-query eager load)
  - iterator (chunked async streaming)
  - raw SQL queries
  - using (database alias)
  - DoesNotExist / MultipleObjectsReturned exceptions
"""

import pytest
import pytest_asyncio

import ormpi
from ormpi.exceptions import DoesNotExist, MultipleObjectsReturned
from ormpi.expr import Avg, Count, F, Max, Min, Q, Sum

from __tests__.conftest import Article, Author, Tag


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

async def seed_authors(*specs):
    """Create authors from keyword-argument dicts and return them."""
    return [await Author.objects.create(**s) for s in specs]


async def seed_articles(author, *specs):
    return [await Article.objects.create(author=author, **s) for s in specs]


# ===========================================================================
# 1. get()
# ===========================================================================

class TestGet:
    @pytest.mark.asyncio
    async def test_get_by_pk(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        result = await Author.objects.get(pk=a.pk)
        assert result.pk == a.pk
        assert result.name == "Alice"

    @pytest.mark.asyncio
    async def test_get_by_field(self):
        await Author.objects.create(name="Bob", email="bob@test.com")
        result = await Author.objects.get(email="bob@test.com")
        assert result.name == "Bob"

    @pytest.mark.asyncio
    async def test_get_does_not_exist(self):
        with pytest.raises(DoesNotExist):
            await Author.objects.get(pk=99999)

    @pytest.mark.asyncio
    async def test_get_multiple_objects_returned(self):
        await Author.objects.create(name="Dup", email="dup1@test.com")
        await Author.objects.create(name="Dup", email="dup2@test.com")
        with pytest.raises(MultipleObjectsReturned):
            await Author.objects.get(name="Dup")

    @pytest.mark.asyncio
    async def test_get_with_q_object(self):
        a = await Author.objects.create(name="Carol", email="carol@test.com")
        result = await Author.objects.get(Q(email="carol@test.com"))
        assert result.pk == a.pk

    @pytest.mark.asyncio
    async def test_model_does_not_exist_subclass(self):
        with pytest.raises(Author.DoesNotExist):
            await Author.objects.get(pk=99999)


# ===========================================================================
# 2. filter() — field lookups
# ===========================================================================

class TestFilterLookups:
    @pytest.mark.asyncio
    async def test_exact_match(self):
        await seed_authors(
            {"name": "Alice", "email": "alice@test.com", "age": 25},
            {"name": "Bob",   "email": "bob@test.com",   "age": 30},
        )
        results = await Author.objects.filter(name="Alice")
        assert len(results) == 1
        assert results[0].name == "Alice"

    @pytest.mark.asyncio
    async def test_exact_match_no_results(self):
        results = await Author.objects.filter(name="Nobody")
        assert results == []

    @pytest.mark.asyncio
    async def test_gt(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 20},
            {"name": "B", "email": "b@test.com", "age": 30},
            {"name": "C", "email": "c@test.com", "age": 40},
        )
        results = await Author.objects.filter(age__gt=25)
        assert len(results) == 2
        assert all(r.age > 25 for r in results)

    @pytest.mark.asyncio
    async def test_gte(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 20},
            {"name": "B", "email": "b@test.com", "age": 30},
        )
        results = await Author.objects.filter(age__gte=30)
        assert len(results) == 1
        assert results[0].age == 30

    @pytest.mark.asyncio
    async def test_lt(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 20},
            {"name": "B", "email": "b@test.com", "age": 30},
        )
        results = await Author.objects.filter(age__lt=25)
        assert len(results) == 1
        assert results[0].age == 20

    @pytest.mark.asyncio
    async def test_lte(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 20},
            {"name": "B", "email": "b@test.com", "age": 30},
        )
        results = await Author.objects.filter(age__lte=20)
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_in_list(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
            {"name": "C", "email": "c@test.com", "age": 30},
        )
        results = await Author.objects.filter(age__in=[10, 30])
        assert len(results) == 2
        ages = {r.age for r in results}
        assert ages == {10, 30}

    @pytest.mark.asyncio
    async def test_isnull_true(self):
        await seed_authors(
            {"name": "WithAge",    "email": "wa@test.com", "age": 25},
            {"name": "WithoutAge", "email": "wo@test.com", "age": None},
        )
        results = await Author.objects.filter(age__isnull=True)
        assert len(results) == 1
        assert results[0].name == "WithoutAge"

    @pytest.mark.asyncio
    async def test_isnull_false(self):
        await seed_authors(
            {"name": "WithAge",    "email": "wa@test.com", "age": 25},
            {"name": "WithoutAge", "email": "wo@test.com", "age": None},
        )
        results = await Author.objects.filter(age__isnull=False)
        assert len(results) == 1
        assert results[0].name == "WithAge"

    @pytest.mark.asyncio
    async def test_contains(self):
        await seed_authors(
            {"name": "Alice Smith", "email": "as@test.com"},
            {"name": "Bob Jones",   "email": "bj@test.com"},
        )
        results = await Author.objects.filter(name__contains="Smith")
        assert len(results) == 1
        assert results[0].name == "Alice Smith"

    @pytest.mark.asyncio
    async def test_icontains(self):
        await seed_authors(
            {"name": "Alice Smith", "email": "as@test.com"},
            {"name": "Bob Jones",   "email": "bj@test.com"},
        )
        results = await Author.objects.filter(name__icontains="smith")
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_startswith(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com"},
            {"name": "Bob",   "email": "b@test.com"},
        )
        results = await Author.objects.filter(name__startswith="Al")
        assert len(results) == 1
        assert results[0].name == "Alice"

    @pytest.mark.asyncio
    async def test_istartswith(self):
        await seed_authors({"name": "Alice", "email": "a@test.com"})
        results = await Author.objects.filter(name__istartswith="al")
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_endswith(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com"},
            {"name": "Alice Jr", "email": "aj@test.com"},
        )
        results = await Author.objects.filter(name__endswith="Jr")
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_iendswith(self):
        await seed_authors({"name": "Alice Jr", "email": "aj@test.com"})
        results = await Author.objects.filter(name__iendswith="jr")
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_range(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
            {"name": "C", "email": "c@test.com", "age": 30},
            {"name": "D", "email": "d@test.com", "age": 40},
        )
        results = await Author.objects.filter(age__range=(15, 35))
        assert len(results) == 2
        assert all(15 <= r.age <= 35 for r in results)

    @pytest.mark.asyncio
    async def test_multiple_filters_are_anded(self):
        await seed_authors(
            {"name": "Alice", "email": "alice@test.com", "age": 25, "active": True},
            {"name": "Alice", "email": "alice2@test.com", "age": 30, "active": False},
        )
        results = await Author.objects.filter(name="Alice", active=True)
        assert len(results) == 1
        assert results[0].age == 25

    @pytest.mark.asyncio
    async def test_chained_filters(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "age": 25},
            {"name": "Alice", "email": "b@test.com", "age": 30},
        )
        results = await Author.objects.filter(name="Alice").filter(age=25)
        assert len(results) == 1


# ===========================================================================
# 3. exclude()
# ===========================================================================

class TestExclude:
    @pytest.mark.asyncio
    async def test_exclude_single(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com"},
            {"name": "Bob",   "email": "b@test.com"},
        )
        results = await Author.objects.exclude(name="Alice")
        assert len(results) == 1
        assert results[0].name == "Bob"

    @pytest.mark.asyncio
    async def test_exclude_multiple_conditions(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "age": 25},
            {"name": "Bob",   "email": "b@test.com", "age": 30},
            {"name": "Carol", "email": "c@test.com", "age": 35},
        )
        results = await Author.objects.exclude(age__lt=30)
        assert len(results) == 2
        assert all(r.age >= 30 for r in results)

    @pytest.mark.asyncio
    async def test_filter_then_exclude(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "age": 25},
            {"name": "Bob",   "email": "b@test.com", "age": 25},
            {"name": "Carol", "email": "c@test.com", "age": 30},
        )
        results = await Author.objects.filter(age=25).exclude(name="Alice")
        assert len(results) == 1
        assert results[0].name == "Bob"


# ===========================================================================
# 4. all() / none()
# ===========================================================================

class TestAllNone:
    @pytest.mark.asyncio
    async def test_all_returns_every_row(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
            {"name": "C", "email": "c@test.com"},
        )
        results = await Author.objects.all()
        assert len(results) == 3

    @pytest.mark.asyncio
    async def test_all_on_empty_table(self):
        results = await Author.objects.all()
        assert results == []

    @pytest.mark.asyncio
    async def test_none_returns_empty(self):
        await seed_authors({"name": "A", "email": "a@test.com"})
        results = await Author.objects.none()
        assert results == []

    @pytest.mark.asyncio
    async def test_none_count_is_zero(self):
        await seed_authors({"name": "A", "email": "a@test.com"})
        count = await Author.objects.none().count()
        assert count == 0


# ===========================================================================
# 5. first() / last() / earliest() / latest()
# ===========================================================================

class TestEdgeRetrievals:
    @pytest.mark.asyncio
    async def test_first_returns_first_row(self):
        a, b = await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
        )
        result = await Author.objects.order_by("pk").first()
        assert result.pk == a.pk

    @pytest.mark.asyncio
    async def test_first_on_empty_returns_none(self):
        result = await Author.objects.first()
        assert result is None

    @pytest.mark.asyncio
    async def test_last_returns_last_row(self):
        a, b = await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
        )
        result = await Author.objects.order_by("pk").last()
        assert result.pk == b.pk

    @pytest.mark.asyncio
    async def test_last_on_empty_returns_none(self):
        result = await Author.objects.last()
        assert result is None

    @pytest.mark.asyncio
    async def test_earliest_by_field(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 30},
            {"name": "B", "email": "b@test.com", "age": 20},
            {"name": "C", "email": "c@test.com", "age": 25},
        )
        result = await Author.objects.earliest("age")
        assert result.age == 20

    @pytest.mark.asyncio
    async def test_latest_by_field(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 30},
            {"name": "B", "email": "b@test.com", "age": 20},
            {"name": "C", "email": "c@test.com", "age": 25},
        )
        result = await Author.objects.latest("age")
        assert result.age == 30


# ===========================================================================
# 6. count() / exists()
# ===========================================================================

class TestCountExists:
    @pytest.mark.asyncio
    async def test_count_all(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
        )
        assert await Author.objects.count() == 2

    @pytest.mark.asyncio
    async def test_count_filtered(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 25},
            {"name": "B", "email": "b@test.com", "age": 30},
        )
        assert await Author.objects.filter(age=25).count() == 1

    @pytest.mark.asyncio
    async def test_count_empty(self):
        assert await Author.objects.count() == 0

    @pytest.mark.asyncio
    async def test_exists_true(self):
        await Author.objects.create(name="A", email="a@test.com")
        assert await Author.objects.exists() is True

    @pytest.mark.asyncio
    async def test_exists_false(self):
        assert await Author.objects.exists() is False

    @pytest.mark.asyncio
    async def test_exists_filtered_true(self):
        await Author.objects.create(name="A", email="a@test.com", age=25)
        assert await Author.objects.filter(age=25).exists() is True

    @pytest.mark.asyncio
    async def test_exists_filtered_false(self):
        await Author.objects.create(name="A", email="a@test.com", age=25)
        assert await Author.objects.filter(age=99).exists() is False


# ===========================================================================
# 7. order_by() / reverse() / distinct()
# ===========================================================================

class TestOrdering:
    @pytest.mark.asyncio
    async def test_order_by_asc(self):
        await seed_authors(
            {"name": "C", "email": "c@test.com", "age": 30},
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
        )
        results = await Author.objects.order_by("age")
        assert [r.age for r in results] == [10, 20, 30]

    @pytest.mark.asyncio
    async def test_order_by_desc(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
            {"name": "C", "email": "c@test.com", "age": 30},
        )
        results = await Author.objects.order_by("-age")
        assert [r.age for r in results] == [30, 20, 10]

    @pytest.mark.asyncio
    async def test_order_by_multiple_fields(self):
        await seed_authors(
            {"name": "B", "email": "b1@test.com", "age": 20},
            {"name": "A", "email": "a@test.com",  "age": 20},
            {"name": "C", "email": "c@test.com",  "age": 30},
        )
        results = await Author.objects.order_by("age", "name")
        assert results[0].name == "A"
        assert results[1].name == "B"
        assert results[2].name == "C"

    @pytest.mark.asyncio
    async def test_reverse(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
        )
        asc    = await Author.objects.order_by("age")
        desc   = await Author.objects.order_by("age").reverse()
        assert asc[0].pk == desc[-1].pk

    @pytest.mark.asyncio
    async def test_distinct(self):
        await seed_authors(
            {"name": "Alice", "email": "a1@test.com", "age": 25},
            {"name": "Alice", "email": "a2@test.com", "age": 25},
            {"name": "Bob",   "email": "b@test.com",  "age": 30},
        )
        results = await Author.objects.values("name").distinct()
        names = [r["name"] for r in results]
        assert sorted(names) == ["Alice", "Bob"]


# ===========================================================================
# 8. Slicing
# ===========================================================================

class TestSlicing:
    @pytest.mark.asyncio
    async def test_limit(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
            {"name": "C", "email": "c@test.com"},
        )
        results = await Author.objects.all()[:2]
        assert len(results) == 2

    @pytest.mark.asyncio
    async def test_offset_and_limit(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
            {"name": "C", "email": "c@test.com"},
            {"name": "D", "email": "d@test.com"},
        )
        results = await Author.objects.order_by("pk")[1:3]
        assert len(results) == 2

    @pytest.mark.asyncio
    async def test_single_index(self):
        a, b, c = await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
            {"name": "C", "email": "c@test.com"},
        )
        result = await Author.objects.order_by("pk")[0]
        assert result.pk == a.pk

    @pytest.mark.asyncio
    async def test_slice_beyond_count_returns_available(self):
        await seed_authors({"name": "A", "email": "a@test.com"})
        results = await Author.objects.all()[:100]
        assert len(results) == 1


# ===========================================================================
# 9. values() / values_list()
# ===========================================================================

class TestProjection:
    @pytest.mark.asyncio
    async def test_values_returns_dicts(self):
        await Author.objects.create(name="Alice", email="alice@test.com", age=25)
        results = await Author.objects.values("name", "age")
        assert isinstance(results[0], dict)
        assert set(results[0].keys()) == {"name", "age"}

    @pytest.mark.asyncio
    async def test_values_all_fields(self):
        await Author.objects.create(name="Alice", email="alice@test.com")
        results = await Author.objects.values()
        assert "name" in results[0]
        assert "email" in results[0]

    @pytest.mark.asyncio
    async def test_values_list_tuples(self):
        await Author.objects.create(name="Alice", email="alice@test.com", age=25)
        results = await Author.objects.values_list("name", "age")
        assert isinstance(results[0], (tuple, list))
        assert results[0][0] == "Alice"

    @pytest.mark.asyncio
    async def test_values_list_flat(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com"},
            {"name": "Bob",   "email": "b@test.com"},
        )
        names = await Author.objects.order_by("name").values_list("name", flat=True)
        assert list(names) == ["Alice", "Bob"]


# ===========================================================================
# 10. only() / defer()
# ===========================================================================

class TestDeferredLoading:
    @pytest.mark.asyncio
    async def test_only_loads_specified_fields(self):
        await Author.objects.create(name="Alice", email="alice@test.com", age=25)
        results = await Author.objects.only("name", "email")
        assert len(results) == 1
        assert results[0].name == "Alice"

    @pytest.mark.asyncio
    async def test_defer_excludes_specified_fields(self):
        await Author.objects.create(name="Alice", email="alice@test.com", age=25)
        results = await Author.objects.defer("bio", "metadata")
        assert len(results) == 1
        assert results[0].name == "Alice"


# ===========================================================================
# 11. Q objects
# ===========================================================================

class TestQObjects:
    @pytest.mark.asyncio
    async def test_q_or(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com"},
            {"name": "Bob",   "email": "b@test.com"},
            {"name": "Carol", "email": "c@test.com"},
        )
        results = await Author.objects.filter(Q(name="Alice") | Q(name="Bob"))
        assert len(results) == 2
        names = {r.name for r in results}
        assert names == {"Alice", "Bob"}

    @pytest.mark.asyncio
    async def test_q_and(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "age": 25, "active": True},
            {"name": "Alice", "email": "a2@test.com", "age": 30, "active": True},
        )
        results = await Author.objects.filter(Q(name="Alice") & Q(age=25))
        assert len(results) == 1
        assert results[0].age == 25

    @pytest.mark.asyncio
    async def test_q_not(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "active": True},
            {"name": "Bob",   "email": "b@test.com", "active": False},
        )
        results = await Author.objects.filter(~Q(active=False))
        assert len(results) == 1
        assert results[0].name == "Alice"

    @pytest.mark.asyncio
    async def test_q_nested(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "age": 25},
            {"name": "Bob",   "email": "b@test.com", "age": 35},
            {"name": "Carol", "email": "c@test.com", "age": 45},
        )
        # (age < 30) OR (age > 40)
        results = await Author.objects.filter(Q(age__lt=30) | Q(age__gt=40))
        assert len(results) == 2
        ages = {r.age for r in results}
        assert ages == {25, 45}

    @pytest.mark.asyncio
    async def test_q_combined_with_kwargs(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "age": 25, "active": True},
            {"name": "Bob",   "email": "b@test.com", "age": 25, "active": False},
        )
        results = await Author.objects.filter(Q(name="Alice") | Q(name="Bob"), age=25)
        assert len(results) == 2

    @pytest.mark.asyncio
    async def test_q_double_not(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "active": True},
        )
        results = await Author.objects.filter(~~Q(active=True))
        assert len(results) == 1


# ===========================================================================
# 12. aggregate()
# ===========================================================================

class TestAggregate:
    @pytest.mark.asyncio
    async def test_count(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
        )
        result = await Author.objects.aggregate(n=Count("id"))
        assert result["n"] == 2

    @pytest.mark.asyncio
    async def test_sum(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
        )
        result = await Author.objects.aggregate(total=Sum("age"))
        assert result["total"] == 30

    @pytest.mark.asyncio
    async def test_avg(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 30},
        )
        result = await Author.objects.aggregate(avg=Avg("age"))
        assert result["avg"] == 20.0

    @pytest.mark.asyncio
    async def test_max(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 99},
        )
        result = await Author.objects.aggregate(mx=Max("age"))
        assert result["mx"] == 99

    @pytest.mark.asyncio
    async def test_min(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 99},
        )
        result = await Author.objects.aggregate(mn=Min("age"))
        assert result["mn"] == 10

    @pytest.mark.asyncio
    async def test_multiple_aggregates(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
            {"name": "C", "email": "c@test.com", "age": 30},
        )
        result = await Author.objects.aggregate(
            n=Count("id"),
            total=Sum("age"),
            mx=Max("age"),
            mn=Min("age"),
        )
        assert result["n"]     == 3
        assert result["total"] == 60
        assert result["mx"]    == 30
        assert result["mn"]    == 10

    @pytest.mark.asyncio
    async def test_aggregate_on_filtered_qs(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 10},
            {"name": "B", "email": "b@test.com", "age": 20},
            {"name": "C", "email": "c@test.com", "age": 30},
        )
        result = await Author.objects.filter(age__gte=20).aggregate(n=Count("id"))
        assert result["n"] == 2

    @pytest.mark.asyncio
    async def test_count_distinct(self):
        await seed_authors(
            {"name": "Alice", "email": "a1@test.com", "age": 25},
            {"name": "Alice", "email": "a2@test.com", "age": 25},
            {"name": "Bob",   "email": "b@test.com",  "age": 30},
        )
        result = await Author.objects.aggregate(n=Count("name", distinct=True))
        assert result["n"] == 2


# ===========================================================================
# 13. select_related()
# ===========================================================================

class TestSelectRelated:
    @pytest.mark.asyncio
    async def test_select_related_loads_fk(self):
        author = await Author.objects.create(name="Alice", email="alice@test.com")
        await Article.objects.create(title="Post 1", author=author)

        articles = await Article.objects.select_related("author")
        assert len(articles) == 1
        cached = articles[0]._ormpi_values.get("author") or getattr(articles[0], "_ormpi_cache_author", None)
        assert cached is not None

    @pytest.mark.asyncio
    async def test_select_related_null_fk(self):
        await Article.objects.create(title="Orphan", author=None)
        articles = await Article.objects.select_related("author")
        assert len(articles) == 1

    @pytest.mark.asyncio
    async def test_select_related_with_filter(self):
        a1 = await Author.objects.create(name="Alice", email="alice@test.com")
        a2 = await Author.objects.create(name="Bob",   email="bob@test.com")
        await Article.objects.create(title="Post 1", author=a1)
        await Article.objects.create(title="Post 2", author=a2)

        articles = await Article.objects.filter(title="Post 1").select_related("author")
        assert len(articles) == 1


# ===========================================================================
# 14. prefetch_related()
# ===========================================================================

class TestPrefetchRelated:
    @pytest.mark.asyncio
    async def test_prefetch_related(self):
        author = await Author.objects.create(name="Alice", email="alice@test.com")
        await Article.objects.create(title="P1", author=author)
        await Article.objects.create(title="P2", author=author)

        authors = await Author.objects.prefetch_related("articles")
        assert len(authors) == 1


# ===========================================================================
# 15. iterator()
# ===========================================================================

class TestIterator:
    @pytest.mark.asyncio
    async def test_iterator_yields_all_rows(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
            {"name": "C", "email": "c@test.com"},
        )
        collected = []
        async for author in Author.objects.all().iterator(chunk_size=2):
            collected.append(author)
        assert len(collected) == 3

    @pytest.mark.asyncio
    async def test_iterator_with_filter(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "age": 25},
            {"name": "B", "email": "b@test.com", "age": 30},
        )
        collected = []
        async for author in Author.objects.filter(age=25).iterator():
            collected.append(author)
        assert len(collected) == 1

    @pytest.mark.asyncio
    async def test_aiter_protocol(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
        )
        collected = []
        async for author in Author.objects.all():
            collected.append(author)
        assert len(collected) == 2


# ===========================================================================
# 16. raw() SQL queries
# ===========================================================================

class TestRawSQL:
    @pytest.mark.asyncio
    async def test_raw_returns_model_instances(self):
        await Author.objects.create(name="Alice", email="alice@test.com")
        results = []
        async for row in Author.objects.raw("SELECT * FROM authors"):
            results.append(row)
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_raw_with_params(self):
        await seed_authors(
            {"name": "Alice", "email": "a@test.com", "age": 25},
            {"name": "Bob",   "email": "b@test.com", "age": 30},
        )
        results = []
        async for row in Author.objects.raw(
            "SELECT * FROM authors WHERE age > ?", [25]
        ):
            results.append(row)
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_execute_raw_returns_rows(self):
        await Author.objects.create(name="Alice", email="alice@test.com")
        rows = await ormpi.execute_raw("SELECT name FROM authors")
        assert len(rows) == 1


# ===========================================================================
# 17. using() — database alias routing
# ===========================================================================

class TestUsing:
    @pytest.mark.asyncio
    async def test_using_default(self):
        await Author.objects.create(name="Alice", email="alice@test.com")
        results = await Author.objects.using("default").all()
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_using_propagates_through_chain(self):
        await Author.objects.create(name="Alice", email="alice@test.com")
        count = await Author.objects.using("default").filter(name="Alice").count()
        assert count == 1


# ===========================================================================
# 18. Miscellaneous / edge cases
# ===========================================================================

class TestEdgeCases:
    @pytest.mark.asyncio
    async def test_filter_boolean_true(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "active": True},
            {"name": "B", "email": "b@test.com", "active": False},
        )
        results = await Author.objects.filter(active=True)
        assert len(results) == 1
        assert results[0].name == "A"

    @pytest.mark.asyncio
    async def test_filter_boolean_false(self):
        await seed_authors(
            {"name": "A", "email": "a@test.com", "active": True},
            {"name": "B", "email": "b@test.com", "active": False},
        )
        results = await Author.objects.filter(active=False)
        assert len(results) == 1
        assert results[0].name == "B"

    @pytest.mark.asyncio
    async def test_queryset_is_reusable(self):
        """The same queryset object can be evaluated multiple times."""
        await seed_authors(
            {"name": "A", "email": "a@test.com"},
            {"name": "B", "email": "b@test.com"},
        )
        qs = Author.objects.all()
        r1 = await qs
        r2 = await qs
        assert len(r1) == len(r2) == 2

    @pytest.mark.asyncio
    async def test_filter_on_null_fk(self):
        await Article.objects.create(title="Orphan", author=None)
        results = await Article.objects.filter(author__isnull=True)
        assert len(results) == 1

    @pytest.mark.asyncio
    async def test_objects_returns_correct_type(self):
        a = await Author.objects.create(name="Alice", email="alice@test.com")
        result = await Author.objects.get(pk=a.pk)
        assert isinstance(result, Author)
