from typing import Any, Protocol, runtime_checkable

@runtime_checkable
class DatabaseRouter(Protocol):
    """Protocol for database routing objects.

    All methods may return ``None`` to defer to the next router.
    """
    def db_for_read(self, model: Any, **hints: Any) -> str | None: ...
    def db_for_write(self, model: Any, **hints: Any) -> str | None: ...
    def allow_relation(self, obj1: Any, obj2: Any, **hints: Any) -> bool | None: ...
    def allow_migrate(
        self,
        db: str,
        model_name: str | None = None,
        **hints: Any,
    ) -> bool | None: ...
