"""
Integration tests — QuerySet API.

Covers: filter, exclude, order_by, values, values_list, count, exists,
bulk_create, bulk_update, update, delete, slicing, Q objects, aggregate.
"""
from __future__ import annotations

import pytest

from ormpi import Q, Count, Sum, Avg, Max, Min

from .conftest import IntAuthor


# ---------------------------------------------------------------------------
# filter() lookups
# ---------------------------------------------------------------------------


async def test_filter_exact(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com")
    await IntAuthor.objects.create(name="Bob", email="b@test.com")
    results = await IntAuthor.objects.filter(name="Alice")
    assert len(results) == 1
    assert results[0]._ormpi_values["name"] == "Alice"


async def test_filter_no_match(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com")
    results = await IntAuthor.objects.filter(name="Nobody")
    assert results == []


async def test_filter_gt_lt(db):
    await IntAuthor.objects.create(name="Young", email="y@test.com", age=20)
    await IntAuthor.objects.create(name="Middle", email="m@test.com", age=35)
    await IntAuthor.objects.create(name="Old", email="o@test.com", age=50)
    results = await IntAuthor.objects.filter(age__gte=30, age__lt=50)
    assert len(results) == 1
    assert results[0]._ormpi_values["name"] == "Middle"


async def test_filter_multiple_ids(db):
    """Filter by multiple PKs using Q OR (IN lookup not yet implemented)."""
    a1 = await IntAuthor.objects.create(name="A", email="a@test.com")
    await IntAuthor.objects.create(name="B", email="b@test.com")
    a3 = await IntAuthor.objects.create(name="C", email="c@test.com")
    results = await IntAuthor.objects.filter(Q(id=a1.pk) | Q(id=a3.pk))
    assert len(results) == 2
    names = {r._ormpi_values["name"] for r in results}
    assert names == {"A", "C"}


async def test_filter_icontains(db):
    await IntAuthor.objects.create(name="Alice Smith", email="a@test.com")
    await IntAuthor.objects.create(name="Bob Jones", email="b@test.com")
    results = await IntAuthor.objects.filter(name__icontains="smith")
    assert len(results) == 1
    assert results[0]._ormpi_values["name"] == "Alice Smith"


# ---------------------------------------------------------------------------
# exclude()
# ---------------------------------------------------------------------------


async def test_exclude(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com")
    await IntAuthor.objects.create(name="Bob", email="b@test.com")
    await IntAuthor.objects.create(name="Carol", email="c@test.com")
    results = await IntAuthor.objects.exclude(name="Alice")
    assert len(results) == 2
    names = {r._ormpi_values["name"] for r in results}
    assert "Alice" not in names


# ---------------------------------------------------------------------------
# Q objects
# ---------------------------------------------------------------------------


async def test_q_or(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com")
    await IntAuthor.objects.create(name="Bob", email="b@test.com")
    await IntAuthor.objects.create(name="Carol", email="c@test.com")
    results = await IntAuthor.objects.filter(Q(name="Alice") | Q(name="Bob"))
    assert len(results) == 2


async def test_q_and(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com", age=25)
    await IntAuthor.objects.create(name="Alice", email="b@test.com", age=30)
    results = await IntAuthor.objects.filter(Q(name="Alice") & Q(age=25))
    assert len(results) == 1
    assert results[0]._ormpi_values["age"] == 25


async def test_q_not(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com", active=True)
    await IntAuthor.objects.create(name="Bob", email="b@test.com", active=False)
    results = await IntAuthor.objects.filter(~Q(active=True))
    assert len(results) == 1
    assert results[0]._ormpi_values["name"] == "Bob"


# ---------------------------------------------------------------------------
# order_by()
# ---------------------------------------------------------------------------


async def test_order_by_asc(db):
    await IntAuthor.objects.create(name="Charlie", email="c@test.com", age=30)
    await IntAuthor.objects.create(name="Alice", email="a@test.com", age=25)
    await IntAuthor.objects.create(name="Bob", email="b@test.com", age=28)
    results = await IntAuthor.objects.order_by("age")
    ages = [r._ormpi_values["age"] for r in results]
    assert ages == sorted(ages)


async def test_order_by_desc(db):
    await IntAuthor.objects.create(name="Charlie", email="c@test.com", age=30)
    await IntAuthor.objects.create(name="Alice", email="a@test.com", age=25)
    await IntAuthor.objects.create(name="Bob", email="b@test.com", age=28)
    results = await IntAuthor.objects.order_by("-age")
    ages = [r._ormpi_values["age"] for r in results]
    assert ages == sorted(ages, reverse=True)


# ---------------------------------------------------------------------------
# count() / exists()
# ---------------------------------------------------------------------------


async def test_count(db):
    await IntAuthor.objects.create(name="A", email="a@test.com")
    await IntAuthor.objects.create(name="B", email="b@test.com")
    assert await IntAuthor.objects.count() == 2


async def test_count_filtered(db):
    await IntAuthor.objects.create(name="A", email="a@test.com", active=True)
    await IntAuthor.objects.create(name="B", email="b@test.com", active=False)
    assert await IntAuthor.objects.filter(active=True).count() == 1


async def test_exists_false_empty(db):
    assert await IntAuthor.objects.exists() is False


async def test_exists_true(db):
    await IntAuthor.objects.create(name="A", email="a@test.com")
    assert await IntAuthor.objects.exists() is True


# ---------------------------------------------------------------------------
# first() / last()
# ---------------------------------------------------------------------------


async def test_first_returns_instance(db):
    await IntAuthor.objects.create(name="Alpha", email="a@test.com")
    await IntAuthor.objects.create(name="Beta", email="b@test.com")
    result = await IntAuthor.objects.order_by("id").first()
    assert result is not None
    assert result._ormpi_values["name"] == "Alpha"


async def test_first_empty_returns_none(db):
    result = await IntAuthor.objects.first()
    assert result is None


# ---------------------------------------------------------------------------
# values()
# ---------------------------------------------------------------------------


async def test_values_returns_dicts(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com")
    results = await IntAuthor.objects.values()
    assert len(results) == 1
    assert isinstance(results[0], dict)
    assert "name" in results[0]
    assert "email" in results[0]


# ---------------------------------------------------------------------------
# Slicing
# ---------------------------------------------------------------------------


async def test_slice_limit(db):
    for i in range(10):
        await IntAuthor.objects.create(name=f"Author {i:02d}", email=f"a{i}@test.com")
    page = await IntAuthor.objects.order_by("id")[:5]
    assert len(page) == 5


async def test_slice_limit_offset(db):
    for i in range(10):
        await IntAuthor.objects.create(name=f"Author {i:02d}", email=f"a{i}@test.com")
    results = await IntAuthor.objects.order_by("id")[2:5]
    assert len(results) == 3


# ---------------------------------------------------------------------------
# update()
# ---------------------------------------------------------------------------


async def test_update_returns_count(db):
    await IntAuthor.objects.create(name="A", email="a@test.com", active=True)
    await IntAuthor.objects.create(name="B", email="b@test.com", active=True)
    count = await IntAuthor.objects.filter(active=True).update(active=False)
    assert count == 2
    assert await IntAuthor.objects.filter(active=True).count() == 0


async def test_update_partial(db):
    await IntAuthor.objects.create(name="Alice", email="a@test.com", age=25)
    await IntAuthor.objects.create(name="Bob", email="b@test.com", age=30)
    count = await IntAuthor.objects.filter(name="Alice").update(age=26)
    assert count == 1
    obj = await IntAuthor.objects.get(name="Alice")
    assert obj._ormpi_values["age"] == 26
    bob = await IntAuthor.objects.get(name="Bob")
    assert bob._ormpi_values["age"] == 30


# ---------------------------------------------------------------------------
# bulk_create()
# ---------------------------------------------------------------------------


async def test_bulk_create(db):
    authors = [IntAuthor(name=f"Author {i}", email=f"a{i}@test.com") for i in range(10)]
    count = await IntAuthor.objects.bulk_create(authors)
    assert count == 10
    assert await IntAuthor.objects.count() == 10


async def test_bulk_create_empty(db):
    count = await IntAuthor.objects.bulk_create([])
    assert count == 0


# ---------------------------------------------------------------------------
# aggregate()
# ---------------------------------------------------------------------------


async def test_aggregate_count(db):
    await IntAuthor.objects.create(name="A", email="a@test.com")
    await IntAuthor.objects.create(name="B", email="b@test.com")
    result = await IntAuthor.objects.aggregate(n=Count("id"))
    assert result["n"] == 2


async def test_aggregate_sum(db):
    await IntAuthor.objects.create(name="A", email="a@test.com", age=10)
    await IntAuthor.objects.create(name="B", email="b@test.com", age=20)
    result = await IntAuthor.objects.aggregate(total=Sum("age"))
    assert result["total"] == 30


async def test_aggregate_avg(db):
    await IntAuthor.objects.create(name="A", email="a@test.com", age=10)
    await IntAuthor.objects.create(name="B", email="b@test.com", age=30)
    result = await IntAuthor.objects.aggregate(avg_age=Avg("age"))
    assert abs(result["avg_age"] - 20.0) < 0.01


async def test_aggregate_max_min(db):
    await IntAuthor.objects.create(name="A", email="a@test.com", age=10)
    await IntAuthor.objects.create(name="B", email="b@test.com", age=50)
    await IntAuthor.objects.create(name="C", email="c@test.com", age=30)
    result = await IntAuthor.objects.aggregate(mx=Max("age"), mn=Min("age"))
    assert result["mx"] == 50
    assert result["mn"] == 10


# ---------------------------------------------------------------------------
# get_or_create() / update_or_create()
# ---------------------------------------------------------------------------


async def test_get_or_create_creates_new(db):
    obj, created = await IntAuthor.objects.get_or_create(
        email="new@test.com", defaults={"name": "New Author"}
    )
    assert created is True
    assert obj._ormpi_values["name"] == "New Author"


async def test_get_or_create_returns_existing(db):
    await IntAuthor.objects.create(name="Existing", email="ex@test.com")
    obj, created = await IntAuthor.objects.get_or_create(email="ex@test.com")
    assert created is False
    assert obj._ormpi_values["name"] == "Existing"


async def test_update_or_create_updates(db):
    await IntAuthor.objects.create(name="Old", email="u@test.com")
    obj, created = await IntAuthor.objects.update_or_create(
        email="u@test.com", defaults={"name": "Updated"}
    )
    assert created is False
    assert obj._ormpi_values["name"] == "Updated"
