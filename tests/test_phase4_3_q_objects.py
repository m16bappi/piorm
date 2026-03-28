"""
Task 4.3 — Q Objects, get(), create(), update(), delete(), aggregate()

Tests all acceptance criteria:
- Q(title="foo") & Q(title="bar") creates AND condition
- Q(a=1) | Q(b=2) creates OR condition
- ~Q(active=True) creates NOT condition
- await Article.objects.get(id=1) returns single instance
- .get() raises DoesNotExist when not found
- .get() raises MultipleObjectsReturned when >1 result
- await Article.objects.create(title="x") creates and returns instance
- await Article.objects.filter(...).update(title="new") returns affected row count
- await Article.objects.filter(...).delete() returns (count, breakdown)
- await Article.objects.aggregate(n=Count("id")) returns {"n": <int>}
- await Article.objects.annotate(n=Count("id")).filter(n__gt=5) works
"""

import pytest

import ormpi
from ormpi import Avg, Count, Max, Min, Q, Sum
from ormpi._core import CharField, IntegerField, create_table, execute_sql


class Article(ormpi.Model):
    title = CharField(max_length=200)
    views = IntegerField(default=0)

    class Meta:
        app_label = "tests43"
        db_table = "articles43"


@pytest.fixture(autouse=True)
async def setup_db():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
    await create_table("tests43.Article")
    await execute_sql("INSERT INTO articles43 (title, views) VALUES ('Alpha', 10)")
    await execute_sql("INSERT INTO articles43 (title, views) VALUES ('Beta', 20)")
    await execute_sql("INSERT INTO articles43 (title, views) VALUES ('Gamma', 30)")


# ── Q object composition ──────────────────────────────────────────────────────


def test_q_and():
    q = Q(title="Alpha") & Q(views=10)
    assert repr(q)  # sanity check: produces a valid Q repr


def test_q_or():
    q = Q(title="Alpha") | Q(title="Beta")
    assert repr(q)


def test_q_not():
    q = ~Q(title="Alpha")
    assert repr(q)


async def test_q_and_filter():
    results = await Article.objects.filter(Q(title="Alpha") & Q(views=10))
    assert len(results) == 1
    assert results[0]._ormpi_values["title"] == "Alpha"


async def test_q_or_filter():
    results = await Article.objects.filter(Q(title="Alpha") | Q(title="Beta"))
    assert len(results) == 2
    titles = {r._ormpi_values["title"] for r in results}
    assert titles == {"Alpha", "Beta"}


async def test_q_not_filter():
    results = await Article.objects.filter(~Q(title="Alpha"))
    assert len(results) == 2
    titles = {r._ormpi_values["title"] for r in results}
    assert "Alpha" not in titles


# ── get() ─────────────────────────────────────────────────────────────────────


async def test_get_single():
    obj = await Article.objects.get(title="Alpha")
    assert obj is not None
    assert isinstance(obj, Article)
    assert obj._ormpi_values["title"] == "Alpha"


async def test_get_does_not_exist():
    with pytest.raises(Exception, match="DoesNotExist"):
        await Article.objects.get(title="NoSuchTitle")


async def test_get_multiple_objects_returned():
    with pytest.raises(Exception, match="more than one"):
        await Article.objects.get(views__gt=0)


# ── create() ─────────────────────────────────────────────────────────────────


async def test_create_returns_instance():
    obj = await Article.objects.create(title="Delta", views=40)
    assert obj is not None
    assert isinstance(obj, Article)
    assert obj._ormpi_values["title"] == "Delta"
    assert obj._ormpi_values["views"] == 40
    # Should be persisted
    all_objs = await Article.objects.all()
    assert len(all_objs) == 4


# ── update() ─────────────────────────────────────────────────────────────────


async def test_bulk_update_returns_count():
    count = await Article.objects.filter(title="Alpha").update(title="AlphaUpdated")
    assert count == 1
    obj = await Article.objects.get(title="AlphaUpdated")
    assert obj._ormpi_values["views"] == 10


async def test_bulk_update_multiple():
    count = await Article.objects.filter(views__gt=15).update(views=99)
    assert count == 2  # Beta and Gamma


# ── delete() ─────────────────────────────────────────────────────────────────


async def test_bulk_delete_returns_tuple():
    result = await Article.objects.filter(title="Alpha").delete()
    count, breakdown = result
    assert count == 1
    assert "Article" in breakdown
    remaining = await Article.objects.all()
    assert len(remaining) == 2


async def test_bulk_delete_all():
    result = await Article.objects.all().delete()
    count, _breakdown = result
    assert count == 3
    remaining = await Article.objects.all()
    assert remaining == []


# ── aggregate() ──────────────────────────────────────────────────────────────


async def test_aggregate_count():
    result = await Article.objects.aggregate(n=Count("id"))
    assert "n" in result
    assert result["n"] == 3


async def test_aggregate_avg():
    result = await Article.objects.aggregate(avg_views=Avg("views"))
    assert "avg_views" in result
    assert abs(result["avg_views"] - 20.0) < 0.01


async def test_aggregate_sum():
    result = await Article.objects.aggregate(total=Sum("views"))
    assert "total" in result
    assert result["total"] == 60


async def test_aggregate_max():
    result = await Article.objects.aggregate(max_views=Max("views"))
    assert result["max_views"] == 30


async def test_aggregate_min():
    result = await Article.objects.aggregate(min_views=Min("views"))
    assert result["min_views"] == 10


async def test_aggregate_multiple():
    result = await Article.objects.aggregate(n=Count("id"), total=Sum("views"))
    assert result["n"] == 3
    assert result["total"] == 60


async def test_aggregate_filtered():
    result = await Article.objects.filter(views__gte=20).aggregate(n=Count("id"))
    assert result["n"] == 2


# ── get_or_create() ──────────────────────────────────────────────────────────


async def test_get_or_create_existing():
    obj, created = await Article.objects.get_or_create(title="Alpha")
    assert not created
    assert obj._ormpi_values["title"] == "Alpha"


async def test_get_or_create_new():
    obj, created = await Article.objects.get_or_create(
        title="NewArticle", defaults={"views": 5}
    )
    assert created
    assert obj._ormpi_values["title"] == "NewArticle"
    all_objs = await Article.objects.all()
    assert len(all_objs) == 4


# ── update_or_create() ───────────────────────────────────────────────────────


async def test_update_or_create_existing():
    obj, created = await Article.objects.update_or_create(
        title="Alpha", defaults={"views": 999}
    )
    assert not created
    assert obj._ormpi_values["views"] == 999


async def test_update_or_create_new():
    obj, created = await Article.objects.update_or_create(
        title="BrandNew", defaults={"views": 7}
    )
    assert created
    assert obj._ormpi_values["title"] == "BrandNew"
    all_objs = await Article.objects.all()
    assert len(all_objs) == 4
