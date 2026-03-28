"""Per-asyncio-task active transaction registry."""
import asyncio

# Maps task id → {alias: Transaction}
_CURRENT: dict = {}


def _get_current_transaction(using: str):
    task = asyncio.current_task()
    if task is None:
        return None
    return _CURRENT.get(id(task), {}).get(using)


def _set_current_transaction(using: str, txn) -> None:
    task = asyncio.current_task()
    if task is None:
        return
    _CURRENT.setdefault(id(task), {})[using] = txn


def _clear_current_transaction(using: str) -> None:
    task = asyncio.current_task()
    if task is None:
        return
    _CURRENT.get(id(task), {}).pop(using, None)
