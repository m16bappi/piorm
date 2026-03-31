# Internal stub for the compiled _core extension.
# Not part of the public API — import from ormpi.* sub-modules instead.
# Python 3.12+ only — no "from __future__ import annotations".

from typing import Any, LiteralString, Literal, Self, final, overload
from collections.abc import AsyncIterator, Coroutine, Generator, Iterator, Sequence

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

# ── QuerySet ─────────────────────────────────────────────────────────────

class QuerySet[M]:
    """Lazy, chainable async query builder.

    Queries are not executed until the queryset is awaited or async-iterated.
    Awaiting produces an ``EvaluatedQuerySet[M]`` with rows cached — see that
    class for synchronous indexing, ``len()``, and iteration.

    Examples::

        # Chain, then await
        qs = Author.objects.filter(active=True).order_by("-created_at")
        results = await qs             # EvaluatedQuerySet[Author]
        results[0].name                # str — direct, no await

        # Async iteration (streams without materialising all rows)
        async for author in Author.objects.order_by("name"):
            print(author.name)

        # Await a single object
        author = await Author.objects.get(pk=1)   # Author
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

    # ── Annotation / projection ───────────────────────────────────────────
    def annotate(self, **kwargs: Any) -> QuerySet[M]: ...
    def values(self, *fields: str) -> QuerySet[dict[str, Any]]: ...
    @overload
    def values_list(self, *fields: str, flat: Literal[True]) -> QuerySet[Any]: ...
    @overload
    def values_list(
        self, *fields: str, flat: Literal[False] = ...
    ) -> QuerySet[tuple[Any, ...]]: ...
    @overload
    def values_list(
        self, *fields: str, flat: bool = ...
    ) -> QuerySet[tuple[Any, ...]] | QuerySet[Any]: ...

    # ── Async protocol ────────────────────────────────────────────────────
    def __aiter__(self) -> AsyncIterator[M]: ...

    # Awaiting fetches all rows and returns an EvaluatedQuerySet (rows cached).
    def __await__(self) -> Generator[Any, None, EvaluatedQuerySet[M]]: ...
    def iterator(self, chunk_size: int = 2000) -> AsyncIterator[M]: ...

    # ── Slicing ───────────────────────────────────────────────────────────
    # Integer index on an *unevaluated* queryset is an async DB fetch.
    # On an *evaluated* queryset use EvaluatedQuerySet.__getitem__ (returns M).
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
        objs: Sequence[M],
        batch_size: int | None = None,
        ignore_conflicts: bool = False,
    ) -> Coroutine[Any, Any, list[M]]: ...
    def bulk_update(
        self,
        objs: Sequence[M],
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

# ── EvaluatedQuerySet ─────────────────────────────────────────────────────

class EvaluatedQuerySet[M](QuerySet[M], Sequence[M]): # type: ignore
    """QuerySet with rows fetched from the database and held in an internal cache.

    Returned by ``await qs``.  Supports synchronous access patterns identical
    to Django's queryset API:

    - ``results[0]``          →  ``M``                       (no ``await`` needed)
    - ``results[0:3]``        →  ``EvaluatedQuerySet[M]``
    - ``len(results)``        →  ``int``
    - ``for r in results``    →  ``M``
    - ``bool(results)``       →  ``bool``
    - ``r in results``        →  ``bool``
    - ``results.filter(...)`` →  ``QuerySet[M]``             (new unevaluated queryset)

    Full example::

        results = await Author.objects.filter(name="Alice")
        assert len(results) == 1
        assert results[0].name == "Alice"          # direct attribute access

        # Still chainable — re-filtering starts a new DB round-trip
        active = await results.filter(active=True)
        assert len(active) <= len(results)
    """

    # Override __getitem__: integer access is synchronous on an evaluated queryset.
    # The parent (QuerySet) has __getitem__(int) -> Coroutine[Any, Any, M];
    # the override here narrows the return to M.
    # type: ignore[override] suppresses the Liskov check for this specific case —
    # the override is safe because after evaluation no async fetch is needed.
    @overload  # type: ignore[override]
    def __getitem__(self, key: int) -> M: ...
    @overload
    def __getitem__(self, key: slice) -> EvaluatedQuerySet[M]: ...  # pyright: ignore[reportIncompatibleMethodOverride]

    # ── Synchronous collection protocol ───────────────────────────────────
    def __iter__(self) -> Iterator[M]: ...
    def __len__(self) -> int: ...
    def __bool__(self) -> bool: ...
    def __contains__(self, item: object) -> bool: ...
    def __reversed__(self) -> Iterator[M]: ...

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
    def annotate(self, **kwargs: Any) -> QuerySet[M]: ...

    # ── Projection (type changes) ─────────────────────────────────────────
    def values(self, *fields: str) -> QuerySet[dict[str, Any]]: ...
    @overload
    def values_list(self, *fields: str, flat: Literal[True]) -> QuerySet[Any]: ...
    @overload
    def values_list(
        self, *fields: str, flat: Literal[False] = ...
    ) -> QuerySet[tuple[Any, ...]]: ...
    @overload
    def values_list(
        self, *fields: str, flat: bool = ...
    ) -> QuerySet[tuple[Any, ...]] | QuerySet[Any]: ...

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
        objs: Sequence[M],
        batch_size: int | None = None,
        ignore_conflicts: bool = False,
    ) -> Coroutine[Any, Any, list[M]]: ...
    def bulk_update(
        self,
        objs: Sequence[M],
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
async def execute_raw(
    sql: LiteralString,
    params: list[Any] | None = None,
    using: str = "default",
) -> list[Any]: ...
def get_version() -> str: ...

# ── DDL helpers ───────────────────────────────────────────────────────────

async def create_table(model_name: str, alias: str = "default") -> None: ...
async def create_all_tables(alias: str = "default") -> None: ...

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
    async def delete(self, using: str = "default") -> tuple[int, dict[str, int]]: ...
    async def refresh_from_db(
        self, fields: list[str] | None = None, using: str = "default"
    ) -> None: ...
    @classmethod
    def from_db(cls, db: str, field_names: list[str], values: list[Any]) -> Self: ...
