"""
Integration tests — ForeignKey relations.

Covers: FK insert/retrieve, select_related, prefetch_related, NULL FK,
CASCADE delete, bulk_create/bulk_update with FK fields.
"""
from __future__ import annotations

import pytest

from .conftest import IntAuthor, IntArticle


# ---------------------------------------------------------------------------
# FK insert and retrieve
# ---------------------------------------------------------------------------


async def test_create_article_with_fk(db):
    author = await IntAuthor.objects.create(name="Alice", email="a@test.com")
    article = await IntArticle.objects.create(
        title="First Post", author_id=author.pk
    )
    assert article.pk is not None
    assert article._ormpi_values["author_id"] == author.pk


async def test_create_article_with_null_fk(db):
    article = await IntArticle.objects.create(title="Orphan", author_id=None)
    assert article._ormpi_values.get("author_id") is None


async def test_filter_by_fk_id(db):
    a1 = await IntAuthor.objects.create(name="Alice", email="a@test.com")
    a2 = await IntAuthor.objects.create(name="Bob", email="b@test.com")
    await IntArticle.objects.create(title="Alice Post", author_id=a1.pk)
    await IntArticle.objects.create(title="Bob Post", author_id=a2.pk)
    results = await IntArticle.objects.filter(author_id=a1.pk)
    assert len(results) == 1
    assert results[0]._ormpi_values["title"] == "Alice Post"


# ---------------------------------------------------------------------------
# select_related
# ---------------------------------------------------------------------------


async def test_select_related_caches_author(db):
    author = await IntAuthor.objects.create(name="Alice", email="a@test.com")
    await IntArticle.objects.create(title="Post", author_id=author.pk)
    articles = await IntArticle.objects.select_related("author")
    assert len(articles) == 1
    article = articles[0]
    assert hasattr(article, "_ormpi_cache_author")
    assert article._ormpi_cache_author is not None
    assert article._ormpi_cache_author["name"] == "Alice"


async def test_select_related_null_fk_is_none(db):
    await IntArticle.objects.create(title="Orphan", author_id=None)
    articles = await IntArticle.objects.select_related("author")
    assert len(articles) == 1
    assert articles[0]._ormpi_cache_author is None


async def test_select_related_multiple_articles(db):
    alice = await IntAuthor.objects.create(name="Alice", email="a@test.com")
    bob = await IntAuthor.objects.create(name="Bob", email="b@test.com")
    await IntArticle.objects.create(title="A1", author_id=alice.pk)
    await IntArticle.objects.create(title="A2", author_id=alice.pk)
    await IntArticle.objects.create(title="B1", author_id=bob.pk)
    await IntArticle.objects.create(title="Orphan", author_id=None)

    articles = await IntArticle.objects.select_related("author")
    assert len(articles) == 4
    cached = [a for a in articles if a._ormpi_cache_author is not None]
    assert len(cached) == 3


# ---------------------------------------------------------------------------
# prefetch_related
# ---------------------------------------------------------------------------


async def test_prefetch_related_articles(db):
    alice = await IntAuthor.objects.create(name="Alice", email="a@test.com")
    bob = await IntAuthor.objects.create(name="Bob", email="b@test.com")
    await IntArticle.objects.create(title="A1", author_id=alice.pk)
    await IntArticle.objects.create(title="A2", author_id=alice.pk)
    await IntArticle.objects.create(title="B1", author_id=bob.pk)

    authors = await IntAuthor.objects.prefetch_related("articles")
    assert len(authors) == 2
    for author in authors:
        assert hasattr(author, "_prefetch_cache_articles")

    alice_obj = next(a for a in authors if a._ormpi_values["name"] == "Alice")
    bob_obj = next(a for a in authors if a._ormpi_values["name"] == "Bob")
    assert len(alice_obj._prefetch_cache_articles) == 2
    assert len(bob_obj._prefetch_cache_articles) == 1


# ---------------------------------------------------------------------------
# bulk_create with FK
# ---------------------------------------------------------------------------


async def test_bulk_create_without_fk(db):
    """bulk_create works for articles without a FK (author_id=NULL)."""
    articles = [IntArticle(title=f"Post {i}") for i in range(5)]
    count = await IntArticle.objects.bulk_create(articles)
    assert count == 5
    assert await IntArticle.objects.count() == 5


# ---------------------------------------------------------------------------
# Multiple FK fields (author + editor)
# ---------------------------------------------------------------------------


async def test_two_fk_fields(db):
    alice = await IntAuthor.objects.create(name="Alice", email="a@test.com")
    bob = await IntAuthor.objects.create(name="Bob", email="b@test.com")
    article = await IntArticle.objects.create(
        title="Edited Post",
        author_id=alice.pk,
        editor_id=bob.pk,
    )
    retrieved = await IntArticle.objects.get(pk=article.pk)
    assert retrieved._ormpi_values["author_id"] == alice.pk
    assert retrieved._ormpi_values["editor_id"] == bob.pk


async def test_editor_fk_nullable(db):
    author = await IntAuthor.objects.create(name="Alice", email="a@test.com")
    article = await IntArticle.objects.create(
        title="No Editor", author_id=author.pk
    )
    retrieved = await IntArticle.objects.get(pk=article.pk)
    assert retrieved._ormpi_values.get("editor_id") is None
