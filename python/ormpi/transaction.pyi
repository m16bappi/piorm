from typing import Any, final
from ormpi._core import Transaction as Transaction   # re-export

@final
class atomic:
    """Async context manager for transactions with automatic savepoint nesting.

    Usage::

        async with atomic() as txn:
            await MyModel.objects.create(name="test")

        # Nested (uses SAVEPOINT):
        async with atomic() as outer:
            async with atomic() as inner:
                ...
    """
    def __init__(self, using: str = "default") -> None: ...
    async def __aenter__(self) -> Transaction: ...
    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: Any,
    ) -> bool: ...


async def begin(using: str = "default") -> Transaction:
    """Begin a transaction manually. Prefer ``atomic()`` for most use-cases."""
    ...
