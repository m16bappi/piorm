from typing import Any, ClassVar
from ormpi._core import Manager, QuerySet
from ormpi.exceptions import DoesNotExist as _DoesNotExist

class ModelOptions:
    """Mirrors the ``class Meta`` inner class."""
    db_table: str
    ordering: list[str]
    unique_together: list[list[str]]
    managed: bool
    abstract: bool

class Model:
    """Base class for all ormpi models.

    Subclass to define a table::

        class Author(Model):
            name  = CharField(max_length=100)
            email = CharField(max_length=200, unique=True)

    Fields are discovered at subclass time and registered in the Rust
    ``ModelRegistry`` automatically.
    """

    # ── Private state (not part of public API) ─────────────────────────
    _ormpi_values: dict[str, Any]
    _ormpi_dirty:  set[str]
    _ormpi_state:  dict[str, Any]

    # ── Class-level attributes ─────────────────────────────────────────
    objects: ClassVar[Manager[Any]]
    DoesNotExist: ClassVar[type[_DoesNotExist]]
    _meta: ClassVar[ModelOptions]

    # ── PK shorthand ──────────────────────────────────────────────────
    @property
    def pk(self) -> Any: ...

    # ── Constructor ───────────────────────────────────────────────────
    def __init__(self, **kwargs: Any) -> None: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

    # ── Persistence ───────────────────────────────────────────────────
    async def save(
        self,
        update_fields: list[str] | None = None,
        using: str = "default",
    ) -> None: ...

    async def delete(self, using: str = "default") -> None: ...

    async def refresh_from_db(
        self,
        fields: list[str] | None = None,
        using: str = "default",
    ) -> None: ...

    # ── Class methods ─────────────────────────────────────────────────
    @classmethod
    def from_db(
        cls,
        db: str,
        field_names: list[str],
        values: list[Any],
    ) -> Model: ...

    class Meta:
        abstract: ClassVar[bool]
        db_table: ClassVar[str]
        ordering: ClassVar[list[str]]
        unique_together: ClassVar[list[list[str]]]
        managed: ClassVar[bool]
