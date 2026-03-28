from typing import Any, final

@final
class ConnectionProxy:
    """Dict-style access to database connection wrappers.

    Usage::

        from ormpi import db
        wrapper = db.connections["default"]
    """
    def __getitem__(self, alias: str) -> Any: ...

connections: ConnectionProxy

async def configure(
    databases: dict[str, Any],
    *,
    auto_sync: bool = False,
) -> None: ...

async def close() -> None: ...
async def create_table(model_name: str, alias: str = "default") -> None: ...
async def create_all_tables(alias: str = "default") -> None: ...
