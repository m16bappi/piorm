"""
Shared fixtures for unit tests in phase 11.3.
"""
from __future__ import annotations

import pytest

import ormpi
from ormpi import CharField, IntegerField
from ormpi._core import create_table


class Author(ormpi.Model):
    name = CharField(max_length=100)
    email = CharField(max_length=200)
    age = IntegerField(null=True)

    class Meta:
        db_table = "unit_authors"


@pytest.fixture
async def db():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
    await create_table("Author")
    yield
    await ormpi.close()
