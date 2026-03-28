"""
Task 6.1 — Transaction Struct & begin/commit/rollback

Tests all acceptance criteria:
- txn = await begin() creates a real database transaction
- await txn.commit() commits and fires on_commit callbacks
- await txn.rollback() rolls back without firing callbacks
- await txn.commit() twice raises TransactionError
- txn.on_commit(fn) registers callback, called only on commit
- on_commit callbacks skipped on rollback
- txn.is_open property reflects state correctly
- Transaction executes actual SQL on the correct database
"""

import pytest

import ormpi
from ormpi._core import CASCADE, CharField, IntegerField, create_table, execute_sql


class Item61(ormpi.Model):
    name = CharField(max_length=100)
    value = IntegerField(default=0)

    class Meta:
        db_table = "items61"


@pytest.fixture(autouse=True)
async def setup_db():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
    await create_table("Item61")


# ── begin / is_open ───────────────────────────────────────────────────────────


async def test_begin_returns_transaction():
    txn = await ormpi.begin()
    assert txn is not None
    assert txn.is_open is True
    await txn.rollback()


async def test_is_open_false_after_commit():
    txn = await ormpi.begin()
    assert txn.is_open is True
    await txn.commit()
    assert txn.is_open is False


async def test_is_open_false_after_rollback():
    txn = await ormpi.begin()
    assert txn.is_open is True
    await txn.rollback()
    assert txn.is_open is False


# ── commit ────────────────────────────────────────────────────────────────────


async def test_commit_persists_data():
    """SQL executed before commit should be visible after commit."""
    txn = await ormpi.begin()
    await txn.execute_raw("INSERT INTO items61 (name, value) VALUES ('committed', 42)")
    await txn.commit()

    rows = await Item61.objects.filter(name="committed")
    assert len(rows) == 1
    assert rows[0]._ormpi_values["value"] == 42


async def test_commit_twice_raises():
    txn = await ormpi.begin()
    await txn.commit()
    with pytest.raises(Exception):
        await txn.commit()


# ── rollback ─────────────────────────────────────────────────────────────────


async def test_rollback_discards_data():
    """SQL executed before rollback should NOT be visible afterwards."""
    txn = await ormpi.begin()
    await txn.execute_raw("INSERT INTO items61 (name, value) VALUES ('rolled_back', 99)")
    await txn.rollback()

    rows = await Item61.objects.filter(name="rolled_back")
    assert len(rows) == 0


async def test_rollback_twice_raises():
    txn = await ormpi.begin()
    await txn.rollback()
    with pytest.raises(Exception):
        await txn.rollback()


# ── on_commit callbacks ───────────────────────────────────────────────────────


async def test_on_commit_fires_after_commit():
    called = []

    txn = await ormpi.begin()
    txn.on_commit(lambda: called.append("fired"))
    await txn.commit()

    assert called == ["fired"]


async def test_on_commit_skipped_on_rollback():
    called = []

    txn = await ormpi.begin()
    txn.on_commit(lambda: called.append("should_not_fire"))
    await txn.rollback()

    assert called == []


async def test_multiple_on_commit_callbacks():
    called = []

    txn = await ormpi.begin()
    txn.on_commit(lambda: called.append(1))
    txn.on_commit(lambda: called.append(2))
    txn.on_commit(lambda: called.append(3))
    await txn.commit()

    assert called == [1, 2, 3]


async def test_on_commit_on_consumed_txn_raises():
    txn = await ormpi.begin()
    await txn.commit()
    with pytest.raises(Exception):
        txn.on_commit(lambda: None)


# ── repr ──────────────────────────────────────────────────────────────────────


async def test_repr_open():
    txn = await ormpi.begin()
    r = repr(txn)
    assert "Transaction" in r
    assert "default" in r
    await txn.rollback()


async def test_repr_consumed():
    txn = await ormpi.begin()
    await txn.commit()
    r = repr(txn)
    assert "consumed" in r or "default" in r
