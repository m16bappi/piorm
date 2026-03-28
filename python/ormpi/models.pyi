from __future__ import annotations

from typing import Any, ClassVar, TypeVar
from ormpi._core import Manager, QuerySet
from ormpi.exceptions import DoesNotExist as _DoesNotExist

M = TypeVar("M", bound="Model")

class Model:
    id: Any
    pk: Any
    objects: ClassVar[Manager[Any]]
    DoesNotExist: ClassVar[type[_DoesNotExist]]

    def __init__(self, **kwargs: Any) -> None: ...
    def __repr__(self) -> str: ...

    async def save(
        self,
        update_fields: list[str] | None = None,
        using: str = "default",
    ) -> None: ...
    async def delete(self, using: str = "default") -> None: ...
    async def refresh_from_db(
        self, fields: list[str] | None = None, using: str = "default"
    ) -> None: ...

    class Meta:
        abstract: ClassVar[bool]
        db_table: ClassVar[str]
        ordering: ClassVar[list[str]]
        unique_together: ClassVar[list[list[str]]]
        managed: ClassVar[bool]
