"""
Shared fixtures for all ormpi tests.

Every test module gets a fresh in-memory SQLite database.  Models are
defined once at module level (registered in Rust on first import) and
their tables are created before each test, then dropped after.
"""

import pytest
import pytest_asyncio
import ormpi
from ormpi.fields import (
    BigIntegerField,
    BooleanField,
    CharField,
    DateField,
    DateTimeField,
    DecimalField,
    FloatField,
    ForeignKey,
    IntegerField,
    JSONField,
    TextField,
    UUIDField,
    CASCADE,
)
from ormpi.db import create_all_tables

# ---------------------------------------------------------------------------
# Models
# ---------------------------------------------------------------------------

class Tag(ormpi.Model):
    name = CharField(max_length=50, unique=True)

    class Meta:
        db_table = "tags"


class Author(ormpi.Model):
    name     = CharField(max_length=100)
    email    = CharField(max_length=200, unique=True)
    age      = IntegerField(null=True)
    score    = FloatField(null=True)
    bio      = TextField(null=True)
    active   = BooleanField(default=True)
    metadata = JSONField(null=True)

    class Meta:
        db_table = "authors"


class Article(ormpi.Model):
    title     = CharField(max_length=200)
    body      = TextField(null=True)
    views     = IntegerField(default=0)
    rating    = FloatField(null=True)
    published = BooleanField(default=False)
    author    = ForeignKey("Author", on_delete=CASCADE, null=True)

    class Meta:
        db_table = "articles"


# ---------------------------------------------------------------------------
# Session-scoped database setup
# ---------------------------------------------------------------------------

DB_CONFIG = {"default": {"ENGINE": "sqlite", "NAME": ":memory:"}}


@pytest_asyncio.fixture(scope="session", autouse=True)
async def _db_session():
    await ormpi.configure(DB_CONFIG)
    yield
    await ormpi.close()


@pytest_asyncio.fixture(autouse=True)
async def _clean_tables():
    """Recreate tables before each test and truncate after."""
    await create_all_tables()
    yield
    # Truncate in FK-safe order
    await ormpi.execute_raw("DELETE FROM articles")
    await ormpi.execute_raw("DELETE FROM authors")
    await ormpi.execute_raw("DELETE FROM tags")
