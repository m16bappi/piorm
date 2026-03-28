# Internal stub for the compiled _core extension.
# Not part of the public API — import from ormpi.* sub-modules instead.
# Python 3.12+ only — no "from __future__ import annotations".

from typing import Any, LiteralString, Literal, Self, final, overload
from collections.abc import AsyncIterator, Coroutine, Generator
import datetime
import decimal
import uuid

# ── Re-exports from expr.pyi (single source of truth) ────────────────────
# Q, F, _FExpr, and Aggregate classes live in expr.pyi.
# Re-exported here so that QuerySet/Manager method signatures reference
# the SAME class that users import from ormpi.expr.

from ormpi.expr import (
    Q as Q,
    F as F,
    _FExpr as _FExpr,
    Aggregate as Aggregate,
    Count as Count,
    Sum as Sum,
    Avg as Avg,
    Max as Max,
    Min as Min,
)

# ── Re-exports from fields.pyi (single source of truth) ──────────────────
from ormpi.fields import (
    Field as Field,
    IntegerField as IntegerField,
    BigIntegerField as BigIntegerField,
    SmallIntegerField as SmallIntegerField,
    PositiveIntegerField as PositiveIntegerField,
    PositiveBigIntegerField as PositiveBigIntegerField,
    PositiveSmallIntegerField as PositiveSmallIntegerField,
    FloatField as FloatField,
    DecimalField as DecimalField,
    CharField as CharField,
    TextField as TextField,
    SlugField as SlugField,
    EmailField as EmailField,
    URLField as URLField,
    IPAddressField as IPAddressField,
    GenericIPAddressField as GenericIPAddressField,
    BooleanField as BooleanField,
    DateField as DateField,
    TimeField as TimeField,
    DateTimeField as DateTimeField,
    DurationField as DurationField,
    UUIDField as UUIDField,
    JSONField as JSONField,
    BinaryField as BinaryField,
    ForeignKey as ForeignKey,
    OneToOneField as OneToOneField,
    ManyToManyField as ManyToManyField,
    CASCADE as CASCADE,
    PROTECT as PROTECT,
    SET_NULL as SET_NULL,
    SET_DEFAULT as SET_DEFAULT,
    DO_NOTHING as DO_NOTHING,
    RESTRICT as RESTRICT,
)

# ── Transaction ───────────────────────────────────────────────────────────

@final
class Transaction:
    """A live database transaction handle."""
    is_open: bool

    async def commit(self) -> None: ...
    async def rollback(self) -> None: ...
    async def execute_raw(
        self,
        sql: LiteralString,
        params: list[Any] | None = None,
    ) -> list[Any]: ...

# ── QuerySet ─────────────────────────────────────────────────────────────

class QuerySet[M]:
    """Lazy, chainable async query builder.

    All filtering/ordering methods return a new ``QuerySet`` — queries are
    not executed until awaited or iterated.

    Awaiting a ``QuerySet`` directly returns ``list[M]``::

        authors = await Author.objects.filter(active=True)
        # authors: list[Author]

    Async iteration::

        async for author in Author.objects.order_by("name"):
            print(author.name)
    """

    # ── Filtering ─────────────────────────────────────────────────────────
    def filter(self, *args: Q, **kwargs: Any) -> QuerySet[M]: ...
    def exclude(self, *args: Q, **kwargs: Any) -> QuerySet[M]: ...
    def all(self) -> QuerySet[M]: ...
    def none(self) -> QuerySet[M]: ...

    # ── Ordering / dedup ──────────────────────────────────────────────────
    def order_by(self, *fields: str) -> QuerySet[M]: ...
    def reverse(self) -> QuerySet[M]: ...
    def distinct(self, *fields: str) -> QuerySet[M]: ...

    # ── Traversal ─────────────────────────────────────────────────────────
    def select_related(self, *fields: str, depth: int | None = None) -> QuerySet[M]: ...
    def prefetch_related(self, *lookups: str) -> QuerySet[M]: ...
    def only(self, *fields: str) -> QuerySet[M]: ...
    def defer(self, *fields: str) -> QuerySet[M]: ...

    # ── Routing ───────────────────────────────────────────────────────────
    def using(self, alias: str) -> QuerySet[M]: ...
    def using_transaction(self, txn: Transaction) -> QuerySet[M]: ...

    # ── Annotation / projection ───────────────────────────────────────────
    def annotate(self, **kwargs: Any) -> QuerySet[M]: ...
    def values(self, *fields: str) -> QuerySet[dict[str, Any]]: ...

    @overload
    def values_list(self, *fields: str, flat: Literal[True]) -> QuerySet[Any]: ...
    @overload
    def values_list(self, *fields: str, flat: Literal[False] = ...) -> QuerySet[tuple[Any, ...]]: ...
    @overload
    def values_list(self, *fields: str, flat: bool = ...) -> QuerySet[tuple[Any, ...]] | QuerySet[Any]: ...

    # ── Async protocol ────────────────────────────────────────────────────
    def __aiter__(self) -> AsyncIterator[M]: ...

    # __await__ allows ``results = await queryset`` → list[M]
    def __await__(self) -> Generator[Any, None, list[M]]: ...

    def iterator(self, chunk_size: int = 2000) -> AsyncIterator[M]: ...

    # ── Slicing ───────────────────────────────────────────────────────────
    @overload
    def __getitem__(self, key: int) -> Coroutine[Any, Any, M]: ...
    @overload
    def __getitem__(self, key: slice) -> QuerySet[M]: ...

    # ── Terminal async ────────────────────────────────────────────────────
    def get(self, *args: Q, **kwargs: Any) -> Coroutine[Any, Any, M]: ...
    def create(self, **kwargs: Any) -> Coroutine[Any, Any, M]: ...
    def get_or_create(
        self,
        defaults: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> Coroutine[Any, Any, tuple[M, bool]]: ...
    def update_or_create(
        self,
        defaults: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> Coroutine[Any, Any, tuple[M, bool]]: ...
    def update(self, **kwargs: Any) -> Coroutine[Any, Any, int]: ...
    def delete(self) -> Coroutine[Any, Any, tuple[int, dict[str, int]]]: ...
    def count(self) -> Coroutine[Any, Any, int]: ...
    def exists(self) -> Coroutine[Any, Any, bool]: ...
    def first(self) -> Coroutine[Any, Any, M | None]: ...
    def last(self) -> Coroutine[Any, Any, M | None]: ...
    def earliest(self, *fields: str) -> Coroutine[Any, Any, M]: ...
    def latest(self, *fields: str) -> Coroutine[Any, Any, M]: ...
    def aggregate(self, **kwargs: Any) -> Coroutine[Any, Any, dict[str, Any]]: ...
    def bulk_create(
        self,
        objs: list[M],
        batch_size: int | None = None,
        ignore_conflicts: bool = False,
    ) -> Coroutine[Any, Any, list[M]]: ...
    def bulk_update(
        self,
        objs: list[M],
        fields: list[str],
        batch_size: int | None = None,
    ) -> Coroutine[Any, Any, int]: ...
    def raw(
        self,
        sql: LiteralString,
        params: list[Any] | None = None,
    ) -> RawQuerySet[M]: ...

# ── RawQuerySet ───────────────────────────────────────────────────────────

class RawQuerySet[M]:
    """Result of ``QuerySet.raw()`` — async-iterable over model instances."""
    def __aiter__(self) -> AsyncIterator[M]: ...

# ── Manager ───────────────────────────────────────────────────────────────

class Manager[M]:
    """Default manager attached to every Model as ``Model.objects``.

    Exposes the full ``QuerySet`` API directly so that the common pattern::

        await Author.objects.filter(active=True).order_by("name")

    works without a preliminary ``.all()`` call.
    """

    # ── Chainable → QuerySet[M] ───────────────────────────────────────────
    def all(self) -> QuerySet[M]: ...
    def none(self) -> QuerySet[M]: ...
    def filter(self, *args: Q, **kwargs: Any) -> QuerySet[M]: ...
    def exclude(self, *args: Q, **kwargs: Any) -> QuerySet[M]: ...
    def order_by(self, *fields: str) -> QuerySet[M]: ...
    def reverse(self) -> QuerySet[M]: ...
    def distinct(self, *fields: str) -> QuerySet[M]: ...
    def select_related(self, *fields: str, depth: int | None = None) -> QuerySet[M]: ...
    def prefetch_related(self, *lookups: str) -> QuerySet[M]: ...
    def only(self, *fields: str) -> QuerySet[M]: ...
    def defer(self, *fields: str) -> QuerySet[M]: ...
    def using(self, alias: str) -> QuerySet[M]: ...
    def using_transaction(self, txn: Transaction) -> QuerySet[M]: ...
    def annotate(self, **kwargs: Any) -> QuerySet[M]: ...

    # ── Projection (type changes) ─────────────────────────────────────────
    def values(self, *fields: str) -> QuerySet[dict[str, Any]]: ...
    @overload
    def values_list(self, *fields: str, flat: Literal[True]) -> QuerySet[Any]: ...
    @overload
    def values_list(self, *fields: str, flat: Literal[False] = ...) -> QuerySet[tuple[Any, ...]]: ...
    @overload
    def values_list(self, *fields: str, flat: bool = ...) -> QuerySet[tuple[Any, ...]] | QuerySet[Any]: ...

    # ── Async iteration ───────────────────────────────────────────────────
    def iterator(self, chunk_size: int = 2000) -> AsyncIterator[M]: ...

    # ── Terminal async ────────────────────────────────────────────────────
    def get(self, *args: Q, **kwargs: Any) -> Coroutine[Any, Any, M]: ...
    def create(self, **kwargs: Any) -> Coroutine[Any, Any, M]: ...
    def get_or_create(
        self,
        defaults: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> Coroutine[Any, Any, tuple[M, bool]]: ...
    def update_or_create(
        self,
        defaults: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> Coroutine[Any, Any, tuple[M, bool]]: ...
    def update(self, **kwargs: Any) -> Coroutine[Any, Any, int]: ...
    def delete(self) -> Coroutine[Any, Any, tuple[int, dict[str, int]]]: ...
    def count(self) -> Coroutine[Any, Any, int]: ...
    def exists(self) -> Coroutine[Any, Any, bool]: ...
    def first(self) -> Coroutine[Any, Any, M | None]: ...
    def last(self) -> Coroutine[Any, Any, M | None]: ...
    def earliest(self, *fields: str) -> Coroutine[Any, Any, M]: ...
    def latest(self, *fields: str) -> Coroutine[Any, Any, M]: ...
    def aggregate(self, **kwargs: Any) -> Coroutine[Any, Any, dict[str, Any]]: ...
    def bulk_create(
        self,
        objs: list[M],
        batch_size: int | None = None,
        ignore_conflicts: bool = False,
    ) -> Coroutine[Any, Any, list[M]]: ...
    def bulk_update(
        self,
        objs: list[M],
        fields: list[str],
        batch_size: int | None = None,
    ) -> Coroutine[Any, Any, int]: ...
    def raw(
        self,
        sql: LiteralString,
        params: list[Any] | None = None,
    ) -> RawQuerySet[M]: ...

# ── Connection proxy ──────────────────────────────────────────────────────

@final
class ConnectionProxy:
    def __getitem__(self, alias: str) -> Any: ...

# ── Top-level async functions ─────────────────────────────────────────────

async def configure(
    databases: dict[str, Any],
    *,
    auto_sync: bool = False,
) -> None: ...

async def close() -> None: ...
async def begin(using: str = "default") -> Transaction: ...
async def execute_raw(
    sql: LiteralString,
    params: list[Any] | None = None,
    using: str = "default",
) -> list[Any]: ...
def get_version() -> str: ...

# ── DDL helpers ───────────────────────────────────────────────────────────

async def create_table(model_name: str, alias: str = "default") -> None: ...
async def create_all_tables(alias: str = "default") -> None: ...

# ── Transaction helpers ───────────────────────────────────────────────────

async def create_savepoint(txn: Transaction) -> str: ...
async def release_savepoint(txn: Transaction, name: str) -> None: ...
async def rollback_to_savepoint(txn: Transaction, name: str) -> None: ...

# ── Internal ──────────────────────────────────────────────────────────────

def _register_model_class(cls: type) -> None: ...
def run_cli(args: list[str]) -> int: ...

# ── Model base (used by models.py) ────────────────────────────────────────
# Full public stub lives in models.pyi.

class Model:
    """Internal Rust Model base.  Full public stub is in models.pyi."""
    def __init__(self, **kwargs: Any) -> None: ...
    async def save(
        self, update_fields: list[str] | None = None, using: str = "default"
    ) -> None: ...
    async def delete(self, using: str = "default") -> None: ...
    async def refresh_from_db(
        self, fields: list[str] | None = None, using: str = "default"
    ) -> None: ...
    @classmethod
    def from_db(cls, db: str, field_names: list[str], values: list[Any]) -> Self: ...
