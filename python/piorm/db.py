"""
Database connection utilities.

    from piorm.db import connections, configure, close
    from piorm import db   # namespace access: db.connections[...]
"""

__all__ = [
    "connections",
    "configure",
    "close",
    "create_table",
    "create_all_tables",
]

from piorm._core import ConnectionProxy as _ConnectionProxy
from piorm._core import create_all_tables as _create_all_tables
from piorm._core import create_table as _create_table
from piorm._core import configure as _configure_core
from piorm._core import close as _close_core

#: Proxy for dict-style access to connection wrappers by alias.
#: Example: ``piorm.db.connections["default"].vendor``
connections: _ConnectionProxy = _ConnectionProxy()


async def configure(
    databases: dict,
    *,
    auto_sync: bool = False,
) -> None:
    """Configure database connections.

    Thin wrapper around the Rust ``_core.configure`` that is also accessible
    as ``piorm.configure()`` from the root package.

    Args:
        databases: Mapping of alias → connection config dict.
        auto_sync: When ``True``, create all model tables automatically
                   using ``CREATE TABLE IF NOT EXISTS`` after pools are ready.
    """
    await _configure_core(databases, auto_sync=auto_sync)


async def close() -> None:
    """Close all database connection pools."""
    await _close_core()


async def create_table(model_name: str, alias: str = "default") -> None:
    """Create the database table for a single registered model."""
    await _create_table(model_name, alias)


async def create_all_tables(alias: str = "default") -> None:
    """Create tables for all registered models on the given database alias."""
    await _create_all_tables(alias)
