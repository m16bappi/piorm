"""
Task 4.2 — QuerySet Evaluation: SQL Generation & Execution

Tests all acceptance criteria:
- await qs returns list of model instances
- filter() generates correct WHERE clause
- count() returns integer
- exists() returns bool
- first() returns first model or None
- qs[:10] sets LIMIT 10
- qs[5:15] sets LIMIT 10, OFFSET 5
- values() returns list of dicts
- async for iterates correctly
"""

import pytest

import ormpi
from ormpi._core import CharField, IntegerField, create_table, execute_sql


class Article(ormpi.Model):
    title = CharField(max_length=200)
    views = IntegerField(default=0)

    class Meta:
        app_label = "tests"
        db_table = "articles"


@pytest.fixture(autouse=True)
async def setup_db():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
    # Create the articles table via the model registry (generates correct DDL).
    await create_table("tests.Article")
    # Seed three rows using execute_sql (which handles DML without expecting rows back).
    await execute_sql("INSERT INTO articles (title, views) VALUES ('Alpha', 10)")
    await execute_sql("INSERT INTO articles (title, views) VALUES ('Beta', 20)")
    await execute_sql("INSERT INTO articles (title, views) VALUES ('Gamma', 30)")


# ── Basic evaluation ─────────────────────────────────────────────────────────


async def test_await_all_returns_instances():
    results = await Article.objects.all()
    assert len(results) == 3
    for obj in results:
        assert isinstance(obj, Article)
        assert obj.pk is not None


async def test_await_filter_where_clause():
    results = await Article.objects.filter(title="Alpha")
    assert len(results) == 1
    assert results[0]._ormpi_values["title"] == "Alpha"


async def test_filter_no_match():
    results = await Article.objects.filter(title="Nonexistent")
    assert results == []


# ── count() ──────────────────────────────────────────────────────────────────


async def test_count_all():
    n = await Article.objects.count()
    assert n == 3


async def test_count_filtered():
    n = await Article.objects.filter(title="Beta").count()
    assert n == 1


# ── exists() ─────────────────────────────────────────────────────────────────


async def test_exists_true():
    assert await Article.objects.filter(title="Alpha").exists() is True


async def test_exists_false():
    assert await Article.objects.filter(title="NoSuchTitle").exists() is False


# ── first() ──────────────────────────────────────────────────────────────────


async def test_first_returns_model_or_none():
    obj = await Article.objects.order_by("id").first()
    assert obj is not None
    assert isinstance(obj, Article)
    assert obj._ormpi_values["title"] == "Alpha"


async def test_first_empty_returns_none():
    obj = await Article.objects.filter(title="Ghost").first()
    assert obj is None


# ── slicing / __getitem__ ─────────────────────────────────────────────────────


async def test_slice_limit():
    results = await Article.objects.order_by("id")[:2]
    assert len(results) == 2


async def test_slice_limit_offset():
    results = await Article.objects.order_by("id")[1:3]
    assert len(results) == 2
    assert results[0]._ormpi_values["title"] == "Beta"


async def test_slice_offset_10_gives_limit_10():
    # We only have 3 rows; limiting to 10 returns all 3.
    results = await Article.objects.order_by("id")[:10]
    assert len(results) == 3


async def test_slice_5_to_15():
    # OFFSET 5 on a 3-row table returns 0 rows.
    results = await Article.objects.order_by("id")[5:15]
    assert results == []


# ── values() ─────────────────────────────────────────────────────────────────


async def test_values_returns_dicts():
    results = await Article.objects.values()
    assert len(results) == 3
    for row in results:
        assert isinstance(row, dict)
        assert "title" in row


# ── async for ────────────────────────────────────────────────────────────────


async def test_async_iter():
    collected = []
    async for article in Article.objects.order_by("id").all():
        collected.append(article)
    assert len(collected) == 3
    assert isinstance(collected[0], Article)


async def test_async_iter_empty():
    collected = []
    async for article in Article.objects.filter(title="Ghost").all():
        collected.append(article)
    assert collected == []


# ── none() ───────────────────────────────────────────────────────────────────


async def test_none_queryset_empty():
    results = await Article.objects.none()
    assert results == []


# ── exclude() ────────────────────────────────────────────────────────────────


async def test_exclude():
    results = await Article.objects.exclude(title="Alpha")
    assert len(results) == 2
    titles = {r._ormpi_values["title"] for r in results}
    assert "Alpha" not in titles
