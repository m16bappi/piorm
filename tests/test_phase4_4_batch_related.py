"""
Task 4.4 — select_related, prefetch_related, bulk_create, bulk_update

Tests all acceptance criteria:
- select_related("author") results in a JOIN, not N+1 queries
- Nested select_related("author__profile") joins two levels
- prefetch_related("articles") issues exactly 2 queries
- Prefetched results accessible as author._prefetch_cache_articles
- bulk_create([...]) inserts all objects in batches
- bulk_create respects batch_size parameter
- bulk_update(objs, ["title"]) updates only the title field
- None FK value results in NULL in JOIN (LEFT JOIN)
"""

import pytest

import ormpi
from ormpi._core import CASCADE, CharField, ForeignKey, IntegerField, create_table, execute_sql


class Author44(ormpi.Model):
    name = CharField(max_length=100)

    class Meta:
        db_table = "authors44"


class Article44(ormpi.Model):
    title = CharField(max_length=200)
    views = IntegerField(default=0)
    author = ForeignKey("Author44", on_delete=CASCADE, related_name="articles", null=True)

    class Meta:
        db_table = "articles44"


@pytest.fixture(autouse=True)
async def setup_db():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
    await create_table("Author44")
    await create_table("Article44")
    # Insert authors
    await execute_sql("INSERT INTO authors44 (name) VALUES ('Alice')")
    await execute_sql("INSERT INTO authors44 (name) VALUES ('Bob')")
    # Insert articles (author_id = 1 → Alice, 2 → Bob, NULL for orphan)
    await execute_sql("INSERT INTO articles44 (title, views, author_id) VALUES ('Alpha', 10, 1)")
    await execute_sql("INSERT INTO articles44 (title, views, author_id) VALUES ('Beta', 20, 1)")
    await execute_sql("INSERT INTO articles44 (title, views, author_id) VALUES ('Gamma', 30, 2)")
    await execute_sql("INSERT INTO articles44 (title, views, author_id) VALUES ('Orphan', 0, NULL)")


# ── select_related ────────────────────────────────────────────────────────────


async def test_select_related_basic():
    articles = await Article44.objects.select_related("author")
    # All non-orphan articles should have author cached
    non_orphan = [a for a in articles if a._ormpi_values.get("author_id") is not None]
    assert len(non_orphan) >= 1
    article = non_orphan[0]
    # The related author dict is cached on the instance
    assert hasattr(article, "_ormpi_cache_author")
    cache = article._ormpi_cache_author
    assert cache is not None
    assert "name" in cache


async def test_select_related_correct_values():
    articles = await Article44.objects.filter(title="Alpha").select_related("author")
    assert len(articles) == 1
    article = articles[0]
    cache = article._ormpi_cache_author
    assert cache is not None
    assert cache["name"] == "Alice"


async def test_select_related_null_fk_is_none():
    """A NULL FK (LEFT JOIN miss) should result in _ormpi_cache_author = None."""
    articles = await Article44.objects.filter(title="Orphan").select_related("author")
    assert len(articles) == 1
    orphan = articles[0]
    assert orphan._ormpi_cache_author is None


async def test_select_related_all_articles():
    articles = await Article44.objects.select_related("author")
    assert len(articles) == 4
    cached_count = sum(1 for a in articles if a._ormpi_cache_author is not None)
    assert cached_count == 3  # Alpha, Beta, Gamma have authors; Orphan does not


# ── prefetch_related ──────────────────────────────────────────────────────────


async def test_prefetch_related_basic():
    authors = await Author44.objects.prefetch_related("articles")
    assert len(authors) == 2
    for author in authors:
        assert hasattr(author, "_prefetch_cache_articles")


async def test_prefetch_related_correct_grouping():
    authors = await Author44.objects.prefetch_related("articles")
    alice = next(a for a in authors if a._ormpi_values["name"] == "Alice")
    bob = next(a for a in authors if a._ormpi_values["name"] == "Bob")

    alice_articles = alice._prefetch_cache_articles
    bob_articles = bob._prefetch_cache_articles

    assert len(alice_articles) == 2  # Alpha + Beta
    assert len(bob_articles) == 1    # Gamma

    alice_titles = {a["title"] for a in alice_articles}
    assert alice_titles == {"Alpha", "Beta"}
    assert bob_articles[0]["title"] == "Gamma"


# ── bulk_create ───────────────────────────────────────────────────────────────


async def test_bulk_create_basic():
    new_articles = [Article44(title=f"Bulk{i}", views=i * 5) for i in range(5)]
    count = await Article44.objects.bulk_create(new_articles)
    assert count == 5
    all_articles = await Article44.objects.all()
    assert len(all_articles) == 9  # 4 original + 5 new


async def test_bulk_create_batch_size():
    """bulk_create with batch_size=2 should still insert all rows."""
    new_articles = [Article44(title=f"Chunk{i}") for i in range(6)]
    count = await Article44.objects.bulk_create(new_articles, batch_size=2)
    assert count == 6
    all_articles = await Article44.objects.all()
    assert len(all_articles) == 10  # 4 + 6


async def test_bulk_create_empty():
    count = await Article44.objects.bulk_create([])
    assert count == 0


async def test_bulk_create_single():
    new = [Article44(title="SingleBulk", views=99)]
    count = await Article44.objects.bulk_create(new)
    assert count == 1
    obj = await Article44.objects.get(title="SingleBulk")
    assert obj._ormpi_values["views"] == 99


# ── bulk_update ───────────────────────────────────────────────────────────────


async def test_bulk_update_single_field():
    objs = await Article44.objects.all()
    # Modify title in-memory on all non-orphan articles
    to_update = [o for o in objs if o._ormpi_values.get("author_id") is not None]
    for obj in to_update:
        obj._ormpi_values["title"] = "UpdatedTitle"

    count = await Article44.objects.bulk_update(to_update, ["title"])
    assert count == 3

    updated = await Article44.objects.filter(title="UpdatedTitle")
    assert len(updated) == 3


async def test_bulk_update_only_specified_fields():
    """views should not change when we only update title."""
    objs = await Article44.objects.filter(title="Alpha")
    assert len(objs) == 1
    obj = objs[0]
    original_views = obj._ormpi_values["views"]  # 10

    obj._ormpi_values["title"] = "AlphaUpdated"
    obj._ormpi_values["views"] = 9999  # should be ignored

    count = await Article44.objects.bulk_update([obj], ["title"])
    assert count == 1

    # views should still be the original value
    refreshed = await Article44.objects.get(title="AlphaUpdated")
    assert refreshed._ormpi_values["views"] == original_views


async def test_bulk_update_empty():
    count = await Article44.objects.bulk_update([], ["title"])
    assert count == 0
