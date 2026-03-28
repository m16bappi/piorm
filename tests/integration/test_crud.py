"""
Integration tests — CRUD operations.

Covers: create, retrieve, save (insert + update), delete, DoesNotExist,
MultipleObjectsReturned. Each test is independent; the `db` fixture
provides a fresh in-memory SQLite database.
"""
from __future__ import annotations

import pytest

import ormpi
from ormpi import MultipleObjectsReturned

from .conftest import IntAuthor


# ---------------------------------------------------------------------------
# create() + get()
# ---------------------------------------------------------------------------


async def test_create_returns_instance_with_pk(db):
    author = await IntAuthor.objects.create(name="Alice", email="alice@test.com")
    assert author.pk is not None
    assert author._ormpi_values["name"] == "Alice"
    assert author._ormpi_values["email"] == "alice@test.com"


async def test_create_and_retrieve_by_pk(db):
    author = await IntAuthor.objects.create(name="Alice", email="alice@test.com")
    retrieved = await IntAuthor.objects.get(pk=author.pk)
    assert retrieved._ormpi_values["name"] == "Alice"
    assert retrieved._ormpi_values["email"] == "alice@test.com"


async def test_create_with_nullable_field(db):
    author = await IntAuthor.objects.create(name="Bob", email="bob@test.com", age=None)
    assert author._ormpi_values.get("age") is None


async def test_create_with_integer_field(db):
    author = await IntAuthor.objects.create(name="Carol", email="carol@test.com", age=30)
    assert author._ormpi_values["age"] == 30


# ---------------------------------------------------------------------------
# save() — insert path
# ---------------------------------------------------------------------------


async def test_save_new_instance(db):
    author = IntAuthor(name="Dave", email="dave@test.com")
    assert author.pk is None
    assert author._ormpi_state["adding"] is True
    await author.save()
    assert author.pk is not None


async def test_save_new_persists(db):
    author = IntAuthor(name="Eve", email="eve@test.com")
    await author.save()
    retrieved = await IntAuthor.objects.get(pk=author.pk)
    assert retrieved._ormpi_values["name"] == "Eve"


# ---------------------------------------------------------------------------
# save() — update path
# ---------------------------------------------------------------------------


async def test_save_update_changes_field(db):
    author = await IntAuthor.objects.create(name="Frank", email="frank@test.com")
    author._ormpi_values["name"] = "Francis"
    author._ormpi_dirty.add("name")
    await author.save()
    refreshed = await IntAuthor.objects.get(pk=author.pk)
    assert refreshed._ormpi_values["name"] == "Francis"


# ---------------------------------------------------------------------------
# delete()
# ---------------------------------------------------------------------------


async def test_delete_removes_instance(db):
    author = await IntAuthor.objects.create(name="Grace", email="grace@test.com")
    pk = author.pk
    count, breakdown = await author.delete()
    assert count == 1
    assert "IntAuthor" in breakdown
    with pytest.raises(Exception, match="DoesNotExist"):
        await IntAuthor.objects.get(pk=pk)


async def test_delete_nonexistent_record_by_filter(db):
    """Deleting a filtered queryset on an empty table returns count=0."""
    count, _ = await IntAuthor.objects.filter(name="Ghost").delete()
    assert count == 0


# ---------------------------------------------------------------------------
# DoesNotExist
# ---------------------------------------------------------------------------


async def test_does_not_exist_on_missing_pk(db):
    with pytest.raises(Exception, match="DoesNotExist"):
        await IntAuthor.objects.get(pk=999999)


async def test_model_specific_does_not_exist(db):
    """Each model class has its own DoesNotExist class attribute."""
    assert issubclass(IntAuthor.DoesNotExist, Exception)
    with pytest.raises(Exception, match="DoesNotExist"):
        await IntAuthor.objects.get(pk=999999)


# ---------------------------------------------------------------------------
# MultipleObjectsReturned
# ---------------------------------------------------------------------------


async def test_multiple_objects_returned(db):
    await IntAuthor.objects.create(name="A", email="a@test.com", active=True)
    await IntAuthor.objects.create(name="B", email="b@test.com", active=True)
    with pytest.raises(Exception, match="more than one"):
        await IntAuthor.objects.get(active=True)
