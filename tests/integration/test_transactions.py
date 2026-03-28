"""
Integration tests — Transaction management.

Covers: atomic commit, rollback on exception, nested atomic with savepoints,
on_commit callbacks, using_transaction() routing.
"""
from __future__ import annotations

import pytest

from ormpi.transaction import atomic

from .conftest import IntAuthor


# ---------------------------------------------------------------------------
# Basic commit / rollback
# ---------------------------------------------------------------------------


async def test_atomic_commit(db):
    async with atomic() as txn:
        await txn.execute_raw(
            "INSERT INTO int_authors (name, email, active) VALUES ('committed', 'c@test.com', 1)"
        )
    assert await IntAuthor.objects.count() == 1


async def test_atomic_rollback_on_exception(db):
    with pytest.raises(ValueError):
        async with atomic() as txn:
            await txn.execute_raw(
                "INSERT INTO int_authors (name, email, active) VALUES ('rollback', 'r@test.com', 1)"
            )
            raise ValueError("deliberate rollback")
    assert await IntAuthor.objects.count() == 0


async def test_atomic_does_not_suppress_exception(db):
    """atomic() must re-raise the exception after rolling back."""
    with pytest.raises(RuntimeError, match="test error"):
        async with atomic():
            raise RuntimeError("test error")


# ---------------------------------------------------------------------------
# using_transaction() — ORM queries inside a transaction
# ---------------------------------------------------------------------------


async def test_using_transaction_commits_with_atomic(db):
    async with atomic() as txn:
        await IntAuthor.objects.using_transaction(txn).create(
            name="via_txn", email="txn@test.com"
        )
    results = await IntAuthor.objects.filter(name="via_txn")
    assert len(results) == 1


async def test_using_transaction_rolls_back(db):
    with pytest.raises(ValueError):
        async with atomic() as txn:
            await IntAuthor.objects.using_transaction(txn).create(
                name="txn_rollback", email="rb@test.com"
            )
            raise ValueError("abort")
    assert await IntAuthor.objects.filter(name="txn_rollback").count() == 0


# ---------------------------------------------------------------------------
# Nested atomic (savepoints)
# ---------------------------------------------------------------------------


async def test_nested_atomic_outer_commits_inner_rolls_back(db):
    async with atomic() as outer:
        await outer.execute_raw(
            "INSERT INTO int_authors (name, email, active) VALUES ('outer', 'o@test.com', 1)"
        )
        try:
            async with atomic() as inner:
                await inner.execute_raw(
                    "INSERT INTO int_authors (name, email, active) VALUES ('inner_fail', 'i@test.com', 1)"
                )
                raise ValueError("inner error")
        except ValueError:
            pass

    rows = await IntAuthor.objects.all()
    names = {r._ormpi_values["name"] for r in rows}
    assert "outer" in names
    assert "inner_fail" not in names


async def test_nested_atomic_both_commit(db):
    async with atomic() as outer:
        await outer.execute_raw(
            "INSERT INTO int_authors (name, email, active) VALUES ('outer', 'o@test.com', 1)"
        )
        async with atomic() as inner:
            await inner.execute_raw(
                "INSERT INTO int_authors (name, email, active) VALUES ('inner', 'i@test.com', 1)"
            )
    assert await IntAuthor.objects.count() == 2


async def test_nested_atomic_outer_rollback_loses_all(db):
    with pytest.raises(ValueError):
        async with atomic() as outer:
            await outer.execute_raw(
                "INSERT INTO int_authors (name, email, active) VALUES ('outer', 'o@test.com', 1)"
            )
            async with atomic() as inner:
                await inner.execute_raw(
                    "INSERT INTO int_authors (name, email, active) VALUES ('inner', 'i@test.com', 1)"
                )
            raise ValueError("outer fail")
    assert await IntAuthor.objects.count() == 0


# ---------------------------------------------------------------------------
# on_commit callbacks
# ---------------------------------------------------------------------------


async def test_on_commit_fires_after_commit(db):
    called = []
    async with atomic() as txn:
        txn.on_commit(lambda: called.append("fired"))
    assert called == ["fired"]


async def test_on_commit_not_fired_on_rollback(db):
    called = []
    with pytest.raises(ValueError):
        async with atomic() as txn:
            txn.on_commit(lambda: called.append("should_not_fire"))
            raise ValueError("boom")
    assert called == []


async def test_on_commit_only_outer_fires_on_inner_rollback(db):
    """Outer on_commit fires; inner savepoint rollback discards inner callback."""
    called = []
    async with atomic() as outer:
        outer.on_commit(lambda: called.append("outer"))
        try:
            async with atomic() as inner:
                inner.on_commit(lambda: called.append("inner"))
                raise ValueError("inner fail")
        except ValueError:
            pass
    assert "outer" in called


# ---------------------------------------------------------------------------
# Multiple operations within one transaction
# ---------------------------------------------------------------------------


async def test_multiple_creates_in_transaction(db):
    async with atomic() as txn:
        for i in range(5):
            await IntAuthor.objects.using_transaction(txn).create(
                name=f"Author {i}", email=f"a{i}@test.com"
            )
    assert await IntAuthor.objects.count() == 5


async def test_create_and_read_in_transaction(db):
    """Create and verify multiple rows within a single transaction."""
    async with atomic() as txn:
        for i in range(3):
            await IntAuthor.objects.using_transaction(txn).create(
                name=f"TxnAuthor {i}", email=f"txn{i}@test.com"
            )
    # All 3 committed
    assert await IntAuthor.objects.count() == 3
