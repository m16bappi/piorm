"""
Integration tests — auto_sync=True in configure().

Verifies that passing auto_sync=True creates all registered model tables
immediately after pool initialisation, without requiring any migration commands.
"""
from __future__ import annotations

import pytest

import ormpi
from ormpi import CASCADE, CharField, ForeignKey, IntegerField, ManyToManyField


# ---------------------------------------------------------------------------
# Test-local models (prefixed to avoid collision with shared conftest models)
# ---------------------------------------------------------------------------


class SyncWidget(ormpi.Model):
    name = CharField(max_length=100)
    count = IntegerField(default=0)

    class Meta:
        db_table = "sync_widgets"


class SyncGadget(ormpi.Model):
    name = CharField(max_length=100)
    widget = ForeignKey(SyncWidget, on_delete=CASCADE, null=True)

    class Meta:
        db_table = "sync_gadgets"


class SyncCategory(ormpi.Model):
    name = CharField(max_length=50)

    class Meta:
        db_table = "sync_categories"


class SyncArticle(ormpi.Model):
    title = CharField(max_length=200)
    categories = ManyToManyField(SyncCategory)

    class Meta:
        db_table = "sync_articles"


_SQLITE_CONFIG = {"default": {"ENGINE": "sqlite", "NAME": ":memory:"}}


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


async def test_auto_sync_creates_tables():
    """Tables must exist after configure(auto_sync=True)."""
    await ormpi.configure(_SQLITE_CONFIG, auto_sync=True)
    # If tables didn't exist, this would raise an error
    widget = await SyncWidget.objects.create(name="Cog", count=3)
    gadget = await SyncGadget.objects.create(name="Gizmo", widget_id=widget.pk)
    assert gadget.pk is not None
    await ormpi.close()


async def test_auto_sync_idempotent():
    """Calling configure(auto_sync=True) twice must not raise."""
    await ormpi.configure(_SQLITE_CONFIG, auto_sync=True)
    await ormpi.close()
    # Second configure — IF NOT EXISTS ensures no error
    await ormpi.configure(_SQLITE_CONFIG, auto_sync=True)
    result = await SyncWidget.objects.count()
    assert result == 0
    await ormpi.close()


async def test_auto_sync_false_does_not_create():
    """auto_sync=False (default) must not create tables."""
    await ormpi.configure(_SQLITE_CONFIG, auto_sync=False)
    with pytest.raises(Exception):
        await SyncWidget.objects.count()  # table does not exist
    await ormpi.close()


async def test_auto_sync_default_is_false():
    """Omitting auto_sync must behave the same as auto_sync=False."""
    await ormpi.configure(_SQLITE_CONFIG)
    with pytest.raises(Exception):
        await SyncWidget.objects.count()  # table does not exist
    await ormpi.close()


async def test_auto_sync_with_fk():
    """FK tables are created and usable after auto_sync=True."""
    await ormpi.configure(_SQLITE_CONFIG, auto_sync=True)
    widget = await SyncWidget.objects.create(name="Base", count=1)
    gadget = await SyncGadget.objects.create(name="Sub", widget_id=widget.pk)
    fetched = await SyncGadget.objects.get(pk=gadget.pk)
    assert fetched.pk == gadget.pk
    await ormpi.close()
