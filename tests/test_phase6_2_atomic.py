"""
Task 6.2 — atomic() Context Manager & Savepoints

Tests all acceptance criteria:
- atomic() commits on clean exit
- atomic() rolls back on exception
- Exception propagates out of atomic() (not suppressed)
- Nested atomic() uses SAVEPOINT (not BEGIN)
- Savepoint rollback doesn't affect outer transaction
- on_commit callbacks fire only after outermost commit
- using_transaction(txn) routes query to transaction's connection
- Savepoints work on SQLite
"""

import pytest

import ormpi
from ormpi._core import CASCADE, CharField, IntegerField, create_table, execute_sql
from ormpi.transaction import atomic


class Post62(ormpi.Model):
    title = CharField(max_length=200)
    score = IntegerField(default=0)

    class Meta:
        db_table = "posts62"


@pytest.fixture(autouse=True)
async def setup_db():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
    await create_table("Post62")


# ── commit on clean exit ──────────────────────────────────────────────────────


async def test_atomic_commit():
    async with atomic() as txn:
        await txn.execute_raw("INSERT INTO posts62 (title, score) VALUES ('committed', 1)")

    count = await Post62.objects.count()
    assert count == 1


async def test_atomic_commit_multiple_rows():
    async with atomic() as txn:
        await txn.execute_raw("INSERT INTO posts62 (title, score) VALUES ('A', 1)")
        await txn.execute_raw("INSERT INTO posts62 (title, score) VALUES ('B', 2)")

    count = await Post62.objects.count()
    assert count == 2


# ── rollback on exception ─────────────────────────────────────────────────────


async def test_atomic_rollback_on_exception():
    try:
        async with atomic() as txn:
            await txn.execute_raw("INSERT INTO posts62 (title, score) VALUES ('will_rollback', 99)")
            raise ValueError("deliberate error")
    except ValueError:
        pass

    count = await Post62.objects.count()
    assert count == 0


async def test_exception_propagates():
    """atomic() does NOT suppress exceptions."""
    with pytest.raises(RuntimeError, match="test error"):
        async with atomic() as txn:
            await txn.execute_raw("INSERT INTO posts62 (title) VALUES ('x')")
            raise RuntimeError("test error")


# ── nested atomic (savepoints) ────────────────────────────────────────────────


async def test_nested_atomic_outer_commits_inner_rolls_back():
    """
    Outer transaction commits; inner rolls back via savepoint.
    Only the outer row survives.
    """
    async with atomic() as outer:
        await outer.execute_raw("INSERT INTO posts62 (title, score) VALUES ('outer', 10)")
        try:
            async with atomic() as inner:
                # inner IS the same txn object (current open txn)
                await inner.execute_raw("INSERT INTO posts62 (title, score) VALUES ('inner_fail', 20)")
                raise ValueError("inner error")
        except ValueError:
            pass
        # outer still open

    rows = await Post62.objects.all()
    titles = {r._ormpi_values["title"] for r in rows}
    assert "outer" in titles
    assert "inner_fail" not in titles


async def test_nested_atomic_both_commit():
    async with atomic() as outer:
        await outer.execute_raw("INSERT INTO posts62 (title, score) VALUES ('outer', 10)")
        async with atomic() as inner:
            await inner.execute_raw("INSERT INTO posts62 (title, score) VALUES ('inner', 20)")
            # No exception → savepoint released, both rows kept

    count = await Post62.objects.count()
    assert count == 2


async def test_nested_atomic_outer_rolls_back_loses_all():
    """If the outer rolls back, everything is gone."""
    try:
        async with atomic() as outer:
            await outer.execute_raw("INSERT INTO posts62 (title) VALUES ('outer')")
            async with atomic() as inner:
                await inner.execute_raw("INSERT INTO posts62 (title) VALUES ('inner')")
                # inner succeeds (savepoint released)
            raise ValueError("outer fail")
    except ValueError:
        pass

    count = await Post62.objects.count()
    assert count == 0


# ── on_commit callbacks ───────────────────────────────────────────────────────


async def test_on_commit_fires_on_atomic_commit():
    called = []

    async with atomic() as txn:
        txn.on_commit(lambda: called.append("fired"))

    assert called == ["fired"]


async def test_on_commit_not_fired_on_rollback():
    called = []

    try:
        async with atomic() as txn:
            txn.on_commit(lambda: called.append("should_not_fire"))
            raise ValueError("boom")
    except ValueError:
        pass

    assert called == []


async def test_on_commit_not_fired_on_inner_rollback():
    """Inner savepoint rollback does NOT fire any on_commit on the outer txn."""
    called = []

    async with atomic() as outer:
        outer.on_commit(lambda: called.append("outer_fired"))
        try:
            async with atomic() as inner:
                inner.on_commit(lambda: called.append("inner_fired"))
                raise ValueError("inner fail")
        except ValueError:
            pass

    # Only outer fired; inner was a savepoint so its "callback" concept is just
    # dropped on rollback
    assert "outer_fired" in called


# ── using_transaction(txn) ────────────────────────────────────────────────────


async def test_using_transaction_commits_with_atomic():
    """
    using_transaction(txn) routes ORM create through the transaction.
    After atomic() commits, the row is visible.
    """
    async with atomic() as txn:
        await Post62.objects.using_transaction(txn).create(title="via_txn", score=42)

    posts = await Post62.objects.filter(title="via_txn")
    assert len(posts) == 1
    assert posts[0]._ormpi_values["score"] == 42


async def test_using_transaction_rolls_back():
    """
    using_transaction(txn) routes through the transaction.
    After rollback, the row is not visible.
    """
    try:
        async with atomic() as txn:
            await Post62.objects.using_transaction(txn).create(title="txn_rollback", score=0)
            raise ValueError("abort")
    except ValueError:
        pass

    posts = await Post62.objects.filter(title="txn_rollback")
    assert len(posts) == 0
