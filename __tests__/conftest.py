"""
Shared fixtures for all ormpi tests.

Session-scoped fixture creates tables once.  Each test cleans up via
DELETE queries so the next test starts with empty tables.
"""

import pytest_asyncio
import ormpi
from ormpi.fields import (
    BooleanField,
    CharField,
    FloatField,
    ForeignKey,
    IntegerField,
    JSONField,
    TextField,
    CASCADE,
)

class Tag(ormpi.Model):
    name = CharField(max_length=50, unique=True)

    class Meta:
        db_table = "tags"


class Author(ormpi.Model):
    name = CharField(max_length=100)
    email = CharField(max_length=200, unique=True)
    age = IntegerField(null=True)
    score = FloatField(null=True)
    bio = TextField(null=True)
    active = BooleanField(default=True)
    metadata = JSONField(null=True)

    class Meta:
        db_table = "authors"


class Article(ormpi.Model):
    title = CharField(max_length=200)
    body = TextField(null=True)
    views = IntegerField(default=0)
    rating = FloatField(null=True)
    published = BooleanField(default=False)
    author = ForeignKey("Author", on_delete=CASCADE, null=True)

    class Meta:
        db_table = "articles"


# ---------------------------------------------------------------------------
# Session-scoped: configure DB + create tables once
# ---------------------------------------------------------------------------

DB_CONFIG = {"default": {"ENGINE": "sqlite", "NAME": ":memory:"}}


@pytest_asyncio.fixture(scope="session", autouse=True)
async def _db_session():
    await ormpi.configure(DB_CONFIG, auto_sync=True)
    yield
    await ormpi.close()


# ---------------------------------------------------------------------------
# Function-scoped: clean tables after each test
# ---------------------------------------------------------------------------


@pytest_asyncio.fixture(autouse=True)
async def _clean_tables():
    yield
    await Article.objects.all().delete()
    await Author.objects.all().delete()
    await Tag.objects.all().delete()
