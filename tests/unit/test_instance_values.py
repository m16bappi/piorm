"""
Task 11.3 — Instance Attribute Population from DB Rows

Tests the full round-trip:
  Write path  : instance.field = value  → _ormpi_values + _ormpi_dirty
  Read path   : instance.field          → _ormpi_values via descriptor __get__
  Hydration   : rows_to_model_instances → _ormpi_values populated from DB row
"""
from __future__ import annotations

import pytest

from .conftest import Author


# ── Descriptor round-trip (no DB required) ───────────────────────────────────


async def test_descriptor_read_write(db):
    author = Author(name="Alice", email="alice@test.com")
    assert author.name == "Alice"        # __get__ reads from _ormpi_values
    assert author.pk is None             # not saved yet

    author.name = "Bob"
    assert author.name == "Bob"
    assert author._ormpi_dirty == set()  # still "adding" — dirty not tracked


# ── Hydration: DB-fetched instance has correct values ────────────────────────


async def test_fetched_instance_has_values(db):
    created = await Author.objects.create(name="Carol", email="carol@test.com", age=28)
    pk = created.pk
    assert pk is not None

    fetched = await Author.objects.get(pk=pk)
    assert fetched.name == "Carol"              # hydrated from DB row via descriptor
    assert fetched.email == "carol@test.com"
    assert fetched.age == 28
    assert fetched._ormpi_state["adding"] is False


# ── Dirty tracking after fetch ────────────────────────────────────────────────


async def test_dirty_tracking_after_fetch(db):
    author = await Author.objects.create(name="Dave", email="dave@test.com")
    fetched = await Author.objects.get(pk=author.pk)

    fetched.name = "David"
    assert "name" in fetched._ormpi_dirty
    assert "email" not in fetched._ormpi_dirty


# ── save() only updates dirty fields ─────────────────────────────────────────


async def test_save_only_updates_dirty(db):
    author = await Author.objects.create(name="Eve", email="eve@test.com", age=30)
    fetched = await Author.objects.get(pk=author.pk)

    # Only touch name — email and age should remain unchanged in the DB.
    fetched.name = "Evelyn"
    assert "name" in fetched._ormpi_dirty
    assert "email" not in fetched._ormpi_dirty
    assert "age" not in fetched._ormpi_dirty

    await fetched.save()

    # Dirty set cleared after successful save.
    assert fetched._ormpi_dirty == set()

    # Verify: re-fetch and confirm only name changed.
    refreshed = await Author.objects.get(pk=author.pk)
    assert refreshed.name == "Evelyn"
    assert refreshed.email == "eve@test.com"
    assert refreshed.age == 30


# ── save() on new instance writes PK back ────────────────────────────────────


async def test_save_new_writes_pk_back(db):
    author = Author(name="Frank", email="frank@test.com")
    assert author.pk is None
    await author.save()
    assert author.pk is not None
    assert isinstance(author.pk, int)
    assert author._ormpi_state["adding"] is False


# ── pk resolves to the correct primary-key field ──────────────────────────────


async def test_pk_resolves_to_primary_key_field(db):
    """author.pk should resolve to whatever field carries primary_key=True."""
    author = Author(name="Grace", email="grace@test.com")
    # Before save: pk is None (auto-increment not yet assigned).
    assert author.pk is None

    await author.save()
    pk_val = author.pk
    assert pk_val is not None
    assert isinstance(pk_val, int)

    # Verify _ormpi_pk_field points to the right attname.
    pk_field_name = Author._ormpi_pk_field
    assert author._ormpi_values[pk_field_name] == pk_val
