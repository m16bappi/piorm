"""
Async transaction utilities: `atomic()` context manager.
"""

__all__ = ["atomic", "begin", "Transaction"]

from typing import Any

from ormpi._core import Transaction as Transaction
from ormpi._core import begin as begin
from ormpi._core import begin as _begin
from ormpi._core import create_savepoint as _create_savepoint
from ormpi._core import release_savepoint as _release_savepoint
from ormpi._core import rollback_to_savepoint as _rollback_to_savepoint

from ormpi._transaction_local import (
    _clear_current_transaction,
    _get_current_transaction,
    _set_current_transaction,
)


class atomic:
    """Async context manager for database transactions with savepoint nesting.

    Usage::

        async with atomic() as txn:
            await MyModel.objects.create(name="test")
            # commits on clean exit, rolls back on exception

        # Nested (uses SAVEPOINT instead of BEGIN):
        async with atomic() as outer:
            async with atomic() as inner:
                ...  # ROLLBACK TO SAVEPOINT on exception

    The yielded ``txn`` is a ``Transaction`` object. If you need to run
    queries inside the transaction use ``using_transaction(txn)`` on a
    queryset, or call ``txn.execute_raw(sql)`` directly.
    """

    def __init__(self, using: str = "default") -> None:
        self._using = using
        self._txn: Transaction | None = None
        self._savepoint_name: str | None = None
        self._is_nested = False

    async def __aenter__(self) -> Transaction:
        current = _get_current_transaction(self._using)
        if current is not None and current.is_open:
            # Nested: use a savepoint instead of BEGIN
            self._is_nested = True
            self._txn = current
            self._savepoint_name = await _create_savepoint(current)
        else:
            # Outermost: begin a real transaction
            self._is_nested = False
            self._txn = await _begin(using=self._using)
            _set_current_transaction(self._using, self._txn)
        assert self._txn is not None
        return self._txn

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: Any,
    ) -> bool:
        assert self._txn is not None
        if exc_type is None:
            # Success path
            if self._is_nested:
                assert self._savepoint_name is not None
                await _release_savepoint(self._txn, self._savepoint_name)
            else:
                await self._txn.commit()
                _clear_current_transaction(self._using)
        else:
            # Exception path — rollback
            if self._is_nested:
                assert self._savepoint_name is not None
                await _rollback_to_savepoint(self._txn, self._savepoint_name)
            else:
                await self._txn.rollback()
                _clear_current_transaction(self._using)
        return False  # never suppress the exception
