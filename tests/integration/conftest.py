"""
Shared fixtures and model definitions for integration tests.

Models use prefixed table names to avoid conflicts with the phase-based unit test models.
"""
from __future__ import annotations

import os

import pytest

import ormpi
from ormpi import (
    CASCADE,
    SET_NULL,
    BooleanField,
    CharField,
    ForeignKey,
    IntegerField,
    TextField,
)
from ormpi._core import create_table


# ---------------------------------------------------------------------------
# Shared test models
# ---------------------------------------------------------------------------


class IntAuthor(ormpi.Model):
    name = CharField(max_length=100)
    email = CharField(max_length=200)
    age = IntegerField(null=True)
    bio = TextField(null=True)
    active = BooleanField(default=True)

    class Meta:
        db_table = "int_authors"


class IntArticle(ormpi.Model):
    title = CharField(max_length=200)
    views = IntegerField(default=0)
    published = BooleanField(default=False)
    author = ForeignKey(
        "IntAuthor",
        on_delete=CASCADE,
        related_name="articles",
        null=True,
    )
    editor = ForeignKey(
        "IntAuthor",
        on_delete=SET_NULL,
        null=True,
        related_name="edited_articles",
    )

    class Meta:
        db_table = "int_articles"


# ---------------------------------------------------------------------------
# Backend parametrization
# ---------------------------------------------------------------------------

BACKENDS = ["sqlite"]
if os.getenv("TEST_POSTGRES"):
    BACKENDS.append("postgres")
if os.getenv("TEST_MYSQL"):
    BACKENDS.append("mysql")


def _db_config(backend: str) -> dict:
    if backend == "sqlite":
        return {"default": {"ENGINE": "sqlite", "NAME": ":memory:"}}
    if backend == "postgres":
        return {
            "default": {
                "ENGINE": "postgres",
                "HOST": os.getenv("POSTGRES_HOST", "localhost"),
                "NAME": "ormpi_test",
                "USER": os.getenv("POSTGRES_USER", "postgres"),
                "PASSWORD": os.getenv("POSTGRES_PASSWORD", "test"),
            }
        }
    if backend == "mysql":
        return {
            "default": {
                "ENGINE": "mysql",
                "HOST": os.getenv("MYSQL_HOST", "localhost"),
                "NAME": "ormpi_test",
                "USER": os.getenv("MYSQL_USER", "root"),
                "PASSWORD": os.getenv("MYSQL_PASSWORD", "test"),
            }
        }
    raise ValueError(f"Unknown backend: {backend}")


@pytest.fixture(params=BACKENDS)
async def db(request):
    """Configure a fresh database and create all integration test tables."""
    config = _db_config(request.param)
    await ormpi.configure(config)
    await create_table("IntAuthor")
    await create_table("IntArticle")
    yield
    await ormpi.close()
