"""Database utilities — DDL helpers for creating model tables."""

from ormpi._core import create_all_tables as _create_all_tables
from ormpi._core import create_table as _create_table


async def create_table(model_name: str, alias: str = "default") -> None:
    """Create the database table for a single model.

    Args:
        model_name: Either ``"app_label.ModelName"`` or just ``"ModelName"``.
        alias: Database alias (default ``"default"``).
    """
    await _create_table(model_name, alias)


async def create_all_tables(alias: str = "default") -> None:
    """Create database tables for all registered models.

    Args:
        alias: Database alias (default ``"default"``).
    """
    await _create_all_tables(alias)
