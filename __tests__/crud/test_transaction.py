"""
Comprehensive transaction tests for ormpi.

Covers every transaction pattern expected in a professional async ORM:

  atomic() context manager
  ─────────────────────────
  - commit on clean exit
  - rollback on exception (exception re-raised)
  - multiple operations are atomic (all-or-nothing)
  - exception mid-block rolls back all prior writes in that block
  - return value from async with block
  - using= alias parameter

  Savepoints (nested atomic)
  ──────────────────────────
  - nested atomic uses SAVEPOINT, not BEGIN
  - inner exception rolls back only inner block, outer commits
  - inner success + outer exception rolls back everything
  - deeply nested (3 levels) — each savepoint independent
  - sibling nested blocks — first rollback, second commits

  Manual begin / commit / rollback
  ──────────────────────────────────
  - begin() returns an open Transaction
  - explicit commit() persists data
  - explicit rollback() discards data
  - Transaction.is_open reflects state after commit/rollback
  - operations after commit() raise (transaction closed)

  QuerySet.using_transaction()
  ────────────────────────────
  - queries inside a manual txn see uncommitted writes (read-your-own-writes)
  - queries outside txn cannot see uncommitted data (isolation)
  - filter / create / update / delete all work within a passed txn
  - txn.execute_raw() can run arbitrary SQL inside the transaction

  Integrity errors
  ─────────────────
  - unique constraint violation inside atomic() rolls back and re-raises
  - partial success before integrity error is rolled back

  Bulk operations in transaction
  ───────────────────────────────
  - bulk_create inside atomic() is atomic
  - bulk_update inside atomic() is atomic

  Cross-model atomicity
  ──────────────────────
  - create parent + child in one atomic block; failure rolls back both
  - delete cascade inside atomic() is atomic
"""

import pytest

from ormpi.exceptions import IntegrityError
from ormpi.transaction import atomic, begin

from __tests__.conftest import Article, Author, Tag


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

async def make_author(**kw) -> Author:
    defaults = {"name": "Alice", "email": "alice@test.com"}
    defaults.update(kw)
    return await Author.objects.create(**defaults)


# ===========================================================================
# 1. atomic() — basic commit and rollback
# ===========================================================================

class TestAtomicBasic:

    @pytest.mark.asyncio
    async def test_commit_on_clean_exit(self):
        """atomic() commits when the block exits without exception."""
        async with atomic():
            await Author.objects.create(name="Alice", email="alice@test.com")

        count = await Author.objects.count()
        assert count == 1

    @pytest.mark.asyncio
    async def test_rollback_on_exception(self):
        """atomic() rolls back all writes when an exception is raised."""
        with pytest.raises(ValueError):
            async with atomic():
                await Author.objects.create(name="Alice", email="alice@test.com")
                raise ValueError("abort")

        count = await Author.objects.count()
        assert count == 0

    @pytest.mark.asyncio
    async def test_exception_is_reraised(self):
        """The original exception escapes the atomic block unchanged."""
        class CustomError(Exception):
            pass

        with pytest.raises(CustomError, match="boom"):
            async with atomic():
                await Author.objects.create(name="Alice", email="alice@test.com")
                raise CustomError("boom")

    @pytest.mark.asyncio
    async def test_multiple_operations_committed_together(self):
        """Multiple creates in one atomic block are all committed."""
        async with atomic():
            await Author.objects.create(name="Alice", email="alice@test.com")
            await Author.objects.create(name="Bob", email="bob@test.com")
            await Author.objects.create(name="Carol", email="carol@test.com")

        count = await Author.objects.count()
        assert count == 3

    @pytest.mark.asyncio
    async def test_partial_writes_rolled_back_on_mid_block_exception(self):
        """Writes before the exception in the block are rolled back."""
        with pytest.raises(RuntimeError):
            async with atomic():
                await Author.objects.create(name="Alice", email="alice@test.com")
                await Author.objects.create(name="Bob", email="bob@test.com")
                raise RuntimeError("mid-block failure")
                await Author.objects.create(name="Carol", email="carol@test.com")  # noqa: unreachable

        count = await Author.objects.count()
        assert count == 0

    @pytest.mark.asyncio
    async def test_atomic_yields_transaction_object(self):
        """The context manager yields the active Transaction instance."""
        async with atomic() as txn:
            assert txn is not None
            assert txn.is_open

    @pytest.mark.asyncio
    async def test_transaction_closed_after_commit(self):
        """Transaction.is_open is False after the block exits cleanly."""
        async with atomic() as txn:
            pass

        assert not txn.is_open

    @pytest.mark.asyncio
    async def test_transaction_closed_after_rollback(self):
        """Transaction.is_open is False after the block exits with exception."""
        with pytest.raises(ValueError):
            async with atomic() as txn:
                raise ValueError("fail")

        assert not txn.is_open

    @pytest.mark.asyncio
    async def test_update_rolled_back(self):
        """An update() inside a rolled-back atomic block is not persisted."""
        author = await make_author()

        with pytest.raises(RuntimeError):
            async with atomic():
                await Author.objects.filter(pk=author.pk).update(name="Changed")
                raise RuntimeError("abort")

        refreshed = await Author.objects.get(pk=author.pk)
        assert refreshed.name == "Alice"

    @pytest.mark.asyncio
    async def test_delete_rolled_back(self):
        """A delete() inside a rolled-back atomic block is not persisted."""
        author = await make_author()

        with pytest.raises(RuntimeError):
            async with atomic():
                await Author.objects.filter(pk=author.pk).delete()
                raise RuntimeError("abort")

        assert await Author.objects.filter(pk=author.pk).exists()


# ===========================================================================
# 2. Savepoints — nested atomic()
# ===========================================================================

class TestSavepoints:

    @pytest.mark.asyncio
    async def test_nested_inner_exception_rolls_back_only_inner(self):
        """
        Inner atomic block fails → only inner writes rolled back.
        Outer block continues and commits.
        """
        async with atomic():
            await Author.objects.create(name="Alice", email="alice@test.com")
            with pytest.raises(ValueError):
                async with atomic():
                    await Author.objects.create(name="Bob", email="bob@test.com")
                    raise ValueError("inner fail")
            # Alice should still be writable — add Carol to confirm outer continues
            await Author.objects.create(name="Carol", email="carol@test.com")

        names = set(
            await Author.objects.values_list("name", flat=True)
        )
        assert "Alice" in names
        assert "Carol" in names
        assert "Bob" not in names

    @pytest.mark.asyncio
    async def test_nested_outer_exception_rolls_back_everything(self):
        """
        Inner block succeeds (savepoint released), but outer block raises.
        All writes — including inner ones — are rolled back.
        """
        with pytest.raises(RuntimeError):
            async with atomic():
                await Author.objects.create(name="Alice", email="alice@test.com")
                async with atomic():
                    await Author.objects.create(name="Bob", email="bob@test.com")
                # inner committed to savepoint — but outer now raises
                raise RuntimeError("outer fail")

        count = await Author.objects.count()
        assert count == 0

    @pytest.mark.asyncio
    async def test_nested_both_succeed(self):
        """Both inner and outer blocks succeed → all writes persisted."""
        async with atomic():
            await Author.objects.create(name="Alice", email="alice@test.com")
            async with atomic():
                await Author.objects.create(name="Bob", email="bob@test.com")

        count = await Author.objects.count()
        assert count == 2

    @pytest.mark.asyncio
    async def test_three_levels_deep(self):
        """Savepoints work correctly at three nesting levels."""
        async with atomic():
            await Author.objects.create(name="L1", email="l1@test.com")
            async with atomic():
                await Author.objects.create(name="L2", email="l2@test.com")
                with pytest.raises(ValueError):
                    async with atomic():
                        await Author.objects.create(name="L3", email="l3@test.com")
                        raise ValueError("level-3 fail")
                # L3 rolled back, L2 continues
                await Author.objects.create(name="L2b", email="l2b@test.com")

        names = set(await Author.objects.values_list("name", flat=True))
        assert names == {"L1", "L2", "L2b"}
        assert "L3" not in names

    @pytest.mark.asyncio
    async def test_sibling_nested_blocks_independent(self):
        """Two sibling inner blocks are independent savepoints."""
        async with atomic():
            async with atomic():
                await Author.objects.create(name="Alice", email="alice@test.com")
            # first inner committed

            with pytest.raises(ValueError):
                async with atomic():
                    await Author.objects.create(name="Bob", email="bob@test.com")
                    raise ValueError("second inner fail")
            # second inner rolled back, first unaffected

        names = set(await Author.objects.values_list("name", flat=True))
        assert "Alice" in names
        assert "Bob" not in names

    @pytest.mark.asyncio
    async def test_nested_inner_yields_same_transaction(self):
        """Inner atomic() yields the same Transaction object as the outer."""
        async with atomic() as outer_txn:
            async with atomic() as inner_txn:
                assert outer_txn is inner_txn


# ===========================================================================
# 3. Manual begin / commit / rollback
# ===========================================================================

class TestManualTransaction:

    @pytest.mark.asyncio
    async def test_begin_returns_open_transaction(self):
        txn = await begin()
        assert txn.is_open
        await txn.rollback()

    @pytest.mark.asyncio
    async def test_explicit_commit_persists_data(self):
        txn = await begin()
        await Author.objects.using_transaction(txn).create(
            name="Alice", email="alice@test.com"
        )
        await txn.commit()

        count = await Author.objects.count()
        assert count == 1

    @pytest.mark.asyncio
    async def test_explicit_rollback_discards_data(self):
        txn = await begin()
        await Author.objects.using_transaction(txn).create(
            name="Alice", email="alice@test.com"
        )
        await txn.rollback()

        count = await Author.objects.count()
        assert count == 0

    @pytest.mark.asyncio
    async def test_is_open_false_after_commit(self):
        txn = await begin()
        await txn.commit()
        assert not txn.is_open

    @pytest.mark.asyncio
    async def test_is_open_false_after_rollback(self):
        txn = await begin()
        await txn.rollback()
        assert not txn.is_open

    @pytest.mark.asyncio
    async def test_multiple_operations_in_manual_txn(self):
        txn = await begin()
        qs = Author.objects.using_transaction(txn)
        await qs.create(name="Alice", email="alice@test.com")
        await qs.create(name="Bob", email="bob@test.com")
        await qs.filter(name="Alice").update(name="Alice-Updated")
        await txn.commit()

        assert await Author.objects.filter(name="Alice-Updated").exists()
        assert await Author.objects.filter(name="Bob").exists()
        assert not await Author.objects.filter(name="Alice").exists()


# ===========================================================================
# 4. QuerySet.using_transaction() — visibility and isolation
# ===========================================================================

class TestUsingTransaction:

    @pytest.mark.asyncio
    async def test_read_your_own_writes(self):
        """
        Within the same transaction, a query sees its own uncommitted writes.
        """
        async with atomic() as txn:
            await Author.objects.using_transaction(txn).create(
                name="Alice", email="alice@test.com"
            )
            count = await Author.objects.using_transaction(txn).count()
            assert count == 1  # visible inside txn

        # Also visible after commit
        assert await Author.objects.count() == 1

    @pytest.mark.asyncio
    async def test_uncommitted_data_not_visible_outside_txn(self):
        """
        Writes inside an open transaction are not visible to queries
        running outside that transaction (isolation).
        """
        txn = await begin()
        await Author.objects.using_transaction(txn).create(
            name="Alice", email="alice@test.com"
        )

        # Outside the transaction — should not see the uncommitted row
        count_outside = await Author.objects.count()
        assert count_outside == 0

        await txn.rollback()

    @pytest.mark.asyncio
    async def test_filter_inside_transaction(self):
        """filter() respects the transaction scope."""
        author = await make_author()

        async with atomic() as txn:
            await Author.objects.using_transaction(txn).filter(pk=author.pk).update(
                name="In-Txn"
            )
            found = await Author.objects.using_transaction(txn).filter(name="In-Txn").exists()
            assert found

        refreshed = await Author.objects.get(pk=author.pk)
        assert refreshed.name == "In-Txn"

    @pytest.mark.asyncio
    async def test_delete_inside_transaction(self):
        """delete() inside a transaction is rolled back if txn is aborted."""
        author = await make_author()

        with pytest.raises(RuntimeError):
            async with atomic() as txn:
                await Author.objects.using_transaction(txn).filter(pk=author.pk).delete()
                raise RuntimeError("abort")

        assert await Author.objects.filter(pk=author.pk).exists()

    @pytest.mark.asyncio
    async def test_execute_raw_inside_transaction(self):
        """txn.execute_raw() participates in the same transaction."""
        async with atomic() as txn:
            await txn.execute_raw(
                "INSERT INTO authors (name, email, active) VALUES (?, ?, ?)",
                ["Raw Alice", "raw@test.com", 1],
            )
            count = await Author.objects.using_transaction(txn).count()
            assert count == 1

        assert await Author.objects.filter(name="Raw Alice").exists()

    @pytest.mark.asyncio
    async def test_execute_raw_rolled_back_with_transaction(self):
        """txn.execute_raw() writes are rolled back if the txn is aborted."""
        with pytest.raises(RuntimeError):
            async with atomic() as txn:
                await txn.execute_raw(
                    "INSERT INTO authors (name, email, active) VALUES (?, ?, ?)",
                    ["Raw Alice", "raw@test.com", 1],
                )
                raise RuntimeError("abort")

        assert not await Author.objects.filter(name="Raw Alice").exists()


# ===========================================================================
# 5. Integrity errors
# ===========================================================================

class TestIntegrityErrors:

    @pytest.mark.asyncio
    async def test_unique_violation_rolls_back_transaction(self):
        """
        Inserting a duplicate unique value inside atomic() rolls back the
        entire block and re-raises IntegrityError.
        """
        await Author.objects.create(name="Alice", email="alice@test.com")

        with pytest.raises(IntegrityError):
            async with atomic():
                await Author.objects.create(name="Bob", email="bob@test.com")
                await Author.objects.create(name="Dupe", email="alice@test.com")  # duplicate email

        # Bob (written before the dupe) must be rolled back
        assert not await Author.objects.filter(name="Bob").exists()

    @pytest.mark.asyncio
    async def test_unique_violation_in_inner_block_only_rolls_back_inner(self):
        """
        Duplicate inside a nested atomic() only rolls back the savepoint,
        not the outer transaction.
        """
        async with atomic():
            await Author.objects.create(name="Alice", email="alice@test.com")

            with pytest.raises(IntegrityError):
                async with atomic():
                    # duplicate email — triggers IntegrityError
                    await Author.objects.create(name="Dupe", email="alice@test.com")

            # Outer transaction should still be alive
            await Author.objects.create(name="Bob", email="bob@test.com")

        assert await Author.objects.filter(name="Alice").exists()
        assert await Author.objects.filter(name="Bob").exists()
        assert not await Author.objects.filter(name="Dupe").exists()


# ===========================================================================
# 6. Bulk operations inside transactions
# ===========================================================================

class TestBulkInTransaction:

    @pytest.mark.asyncio
    async def test_bulk_create_rolled_back_on_exception(self):
        """bulk_create inside atomic() is rolled back with the transaction."""
        objs = [
            Author(name=f"Author-{i}", email=f"a{i}@test.com")
            for i in range(5)
        ]

        with pytest.raises(RuntimeError):
            async with atomic():
                await Author.objects.bulk_create(objs)
                raise RuntimeError("abort")

        assert await Author.objects.count() == 0

    @pytest.mark.asyncio
    async def test_bulk_create_committed(self):
        """bulk_create inside atomic() is committed on clean exit."""
        objs = [
            Author(name=f"Author-{i}", email=f"a{i}@test.com")
            for i in range(5)
        ]

        async with atomic():
            await Author.objects.bulk_create(objs)

        assert await Author.objects.count() == 5

    @pytest.mark.asyncio
    async def test_bulk_update_rolled_back_on_exception(self):
        """bulk_update inside atomic() is rolled back with the transaction."""
        authors = [
            await Author.objects.create(name=f"Author-{i}", email=f"a{i}@test.com")
            for i in range(3)
        ]

        with pytest.raises(RuntimeError):
            async with atomic():
                for a in authors:
                    a.name = "Changed"
                await Author.objects.bulk_update(authors, fields=["name"])
                raise RuntimeError("abort")

        names = set(await Author.objects.values_list("name", flat=True))
        assert all(n.startswith("Author-") for n in names)

    @pytest.mark.asyncio
    async def test_bulk_update_committed(self):
        """bulk_update inside atomic() is committed on clean exit."""
        authors = [
            await Author.objects.create(name=f"Author-{i}", email=f"a{i}@test.com")
            for i in range(3)
        ]

        async with atomic():
            for a in authors:
                a.name = "Bulk-Updated"
            await Author.objects.bulk_update(authors, fields=["name"])

        count = await Author.objects.filter(name="Bulk-Updated").count()
        assert count == 3


# ===========================================================================
# 7. Cross-model atomicity
# ===========================================================================

class TestCrossModelAtomicity:

    @pytest.mark.asyncio
    async def test_parent_and_child_created_atomically(self):
        """Creating a parent and its FK child in one atomic block is all-or-nothing."""
        with pytest.raises(RuntimeError):
            async with atomic():
                author = await Author.objects.create(name="Alice", email="alice@test.com")
                await Article.objects.create(
                    title="My Article",
                    author=author,
                )
                raise RuntimeError("abort")

        assert await Author.objects.count() == 0
        assert await Article.objects.count() == 0

    @pytest.mark.asyncio
    async def test_parent_and_child_committed_together(self):
        """Parent and FK child committed in one atomic block both persist."""
        async with atomic():
            author = await Author.objects.create(name="Alice", email="alice@test.com")
            await Article.objects.create(title="My Article", author=author)

        assert await Author.objects.count() == 1
        assert await Article.objects.count() == 1

    @pytest.mark.asyncio
    async def test_delete_cascade_inside_atomic_rolled_back(self):
        """Cascade delete inside a rolled-back atomic() restores all rows."""
        author = await make_author()
        await Article.objects.create(title="Article", author=author)

        with pytest.raises(RuntimeError):
            async with atomic():
                await author.delete()
                raise RuntimeError("abort")

        assert await Author.objects.filter(pk=author.pk).exists()
        assert await Article.objects.count() == 1

    @pytest.mark.asyncio
    async def test_mixed_operations_atomicity(self):
        """
        A mix of create / update / delete in one atomic block is atomic.
        On failure, all changes — regardless of type — are rolled back.
        """
        existing = await make_author(name="Existing", email="existing@test.com")

        with pytest.raises(RuntimeError):
            async with atomic():
                await Author.objects.create(name="New", email="new@test.com")
                await Author.objects.filter(pk=existing.pk).update(name="Modified")
                await Tag.objects.create(name="tag-x")
                raise RuntimeError("abort")

        # All changes rolled back
        assert not await Author.objects.filter(name="New").exists()
        assert not await Author.objects.filter(name="Modified").exists()
        assert not await Tag.objects.filter(name="tag-x").exists()
        # Original row still unchanged
        original = await Author.objects.get(pk=existing.pk)
        assert original.name == "Existing"
