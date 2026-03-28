# python/ormpi/fields.pyi
# Stub for ormpi field descriptors — Python 3.12+ only.
# No "from __future__ import annotations".

from typing import Any, Literal, Self, overload
from collections.abc import Sequence
import datetime
import decimal
import uuid

# ── on_delete constants ───────────────────────────────────────────────────

type OnDelete = Literal["CASCADE", "PROTECT", "SET_NULL", "SET_DEFAULT", "DO_NOTHING", "RESTRICT"]

CASCADE:     OnDelete
PROTECT:     OnDelete
SET_NULL:    OnDelete
SET_DEFAULT: OnDelete
DO_NOTHING:  OnDelete
RESTRICT:    OnDelete

# ── Choices alias ─────────────────────────────────────────────────────────

type Choices[T] = Sequence[tuple[T, str]]

# ── Base Field[T] ─────────────────────────────────────────────────────────

class Field[T]:
    """
    Generic base descriptor.  T is the Python-level value type of the field.

    - Class access  (``Author.name``):   returns ``Self`` (the descriptor object)
    - Instance access (``author.name``): returns ``T``
    - Assignment (``author.name = v``):  accepts ``T``
    """

    # Introspection attributes (set by contribute_to_class)
    name:        str
    attname:     str
    null:        bool
    unique:      bool
    primary_key: bool
    db_index:    bool
    editable:    bool
    column:      str   # actual SQL column name

    def contribute_to_class(self, cls: type, name: str) -> None: ...

    # Descriptor protocol — typed on T
    @overload
    def __get__(self, obj: None, objtype: type | None = None) -> Self: ...
    @overload
    def __get__(self, obj: object, objtype: type | None = None) -> T: ...

    def __set__(self, obj: object, value: T) -> None: ...

    def db_column(self) -> str: ...

# ── Integer fields ────────────────────────────────────────────────────────
#
# IntegerField[int]          when null=False (default)
# IntegerField[int | None]   when null=True

class IntegerField[T: int | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        default: int | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        primary_key: bool = ...,
        db_column: str | None = ...,
        auto_increment: bool = ...,
    ) -> IntegerField[int | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: int | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        primary_key: bool = ...,
        db_column: str | None = ...,
        auto_increment: bool = ...,
    ) -> IntegerField[int]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: int | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        primary_key: bool = ...,
        db_column: str | None = ...,
        auto_increment: bool = ...,
    ) -> IntegerField[int]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: int | None = None,
        unique: bool = False,
        db_index: bool = False,
        primary_key: bool = False,
        db_column: str | None = None,
        auto_increment: bool = False,
    ) -> None: ...


class BigIntegerField[T: int | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        default: int | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        primary_key: bool = ...,
        db_column: str | None = ...,
        auto_increment: bool = ...,
    ) -> BigIntegerField[int | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: int | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        primary_key: bool = ...,
        db_column: str | None = ...,
        auto_increment: bool = ...,
    ) -> BigIntegerField[int]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: int | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        primary_key: bool = ...,
        db_column: str | None = ...,
        auto_increment: bool = ...,
    ) -> BigIntegerField[int]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: int | None = None,
        unique: bool = False,
        db_index: bool = False,
        primary_key: bool = False,
        db_column: str | None = None,
        auto_increment: bool = False,
    ) -> None: ...


class SmallIntegerField[T: int | None](IntegerField[T]):
    @overload
    def __new__(cls, *, null: Literal[True],  **kwargs: Any) -> SmallIntegerField[int | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, *, null: Literal[False] = ..., **kwargs: Any) -> SmallIntegerField[int]: ...
    @overload
    def __new__(cls, *, null: bool = ..., **kwargs: Any) -> SmallIntegerField[int]: ...

class PositiveIntegerField[T: int | None](IntegerField[T]):
    @overload
    def __new__(cls, *, null: Literal[True],  **kwargs: Any) -> PositiveIntegerField[int | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, *, null: Literal[False] = ..., **kwargs: Any) -> PositiveIntegerField[int]: ...
    @overload
    def __new__(cls, *, null: bool = ..., **kwargs: Any) -> PositiveIntegerField[int]: ...

class PositiveBigIntegerField[T: int | None](BigIntegerField[T]):
    @overload
    def __new__(cls, *, null: Literal[True],  **kwargs: Any) -> PositiveBigIntegerField[int | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, *, null: Literal[False] = ..., **kwargs: Any) -> PositiveBigIntegerField[int]: ...
    @overload
    def __new__(cls, *, null: bool = ..., **kwargs: Any) -> PositiveBigIntegerField[int]: ...

class PositiveSmallIntegerField[T: int | None](SmallIntegerField[T]):
    @overload
    def __new__(cls, *, null: Literal[True],  **kwargs: Any) -> PositiveSmallIntegerField[int | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, *, null: Literal[False] = ..., **kwargs: Any) -> PositiveSmallIntegerField[int]: ...
    @overload
    def __new__(cls, *, null: bool = ..., **kwargs: Any) -> PositiveSmallIntegerField[int]: ...

# ── Float / Decimal ───────────────────────────────────────────────────────

class FloatField[T: float | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        default: float | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> FloatField[float | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: float | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> FloatField[float]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: float | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> FloatField[float]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: float | None = None,
        unique: bool = False,
        db_index: bool = False,
        db_column: str | None = None,
    ) -> None: ...


class DecimalField[T: decimal.Decimal | None](Field[T]):
    max_digits:     int
    decimal_places: int

    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        max_digits: int,
        decimal_places: int,
        *,
        null: Literal[True],
        default: decimal.Decimal | str | None = ...,
        db_column: str | None = ...,
    ) -> DecimalField[decimal.Decimal | None]: ...
    @overload
    def __new__(
        cls,
        max_digits: int,
        decimal_places: int,
        *,
        null: Literal[False] = ...,
        default: decimal.Decimal | str | None = ...,
        db_column: str | None = ...,
    ) -> DecimalField[decimal.Decimal]: ...
    @overload
    def __new__(
        cls,
        max_digits: int,
        decimal_places: int,
        *,
        null: bool = ...,
        default: decimal.Decimal | str | None = ...,
        db_column: str | None = ...,
    ) -> DecimalField[decimal.Decimal]: ...
    def __init__(
        self,
        max_digits: int,
        decimal_places: int,
        *,
        null: bool = False,
        default: decimal.Decimal | str | None = None,
        db_column: str | None = None,
    ) -> None: ...

# ── String fields ─────────────────────────────────────────────────────────

class CharField[T: str | None](Field[T]):
    max_length: int

    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        max_length: int,
        *,
        null: Literal[True],
        default: str | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
        choices: Choices[str] | None = ...,
    ) -> CharField[str | None]: ...
    @overload
    def __new__(
        cls,
        max_length: int,
        *,
        null: Literal[False] = ...,
        default: str | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
        choices: Choices[str] | None = ...,
    ) -> CharField[str]: ...
    @overload
    def __new__(
        cls,
        max_length: int,
        *,
        null: bool = ...,
        default: str | None = ...,
        unique: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
        choices: Choices[str] | None = ...,
    ) -> CharField[str]: ...
    def __init__(
        self,
        max_length: int,
        *,
        null: bool = False,
        default: str | None = None,
        unique: bool = False,
        db_index: bool = False,
        db_column: str | None = None,
        choices: Choices[str] | None = None,
    ) -> None: ...


class TextField[T: str | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        default: str | None = ...,
        db_column: str | None = ...,
    ) -> TextField[str | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: str | None = ...,
        db_column: str | None = ...,
    ) -> TextField[str]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: str | None = ...,
        db_column: str | None = ...,
    ) -> TextField[str]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: str | None = None,
        db_column: str | None = None,
    ) -> None: ...


class SlugField[T: str | None](CharField[T]):
    @overload
    def __new__(cls, max_length: int = ..., *, null: Literal[True],  **kwargs: Any) -> SlugField[str | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, max_length: int = ..., *, null: Literal[False] = ..., **kwargs: Any) -> SlugField[str]: ...
    @overload
    def __new__(cls, max_length: int = ..., *, null: bool = ..., **kwargs: Any) -> SlugField[str]: ...

class EmailField[T: str | None](CharField[T]):
    @overload
    def __new__(cls, max_length: int = ..., *, null: Literal[True],  **kwargs: Any) -> EmailField[str | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, max_length: int = ..., *, null: Literal[False] = ..., **kwargs: Any) -> EmailField[str]: ...
    @overload
    def __new__(cls, max_length: int = ..., *, null: bool = ..., **kwargs: Any) -> EmailField[str]: ...

class URLField[T: str | None](CharField[T]):
    @overload
    def __new__(cls, max_length: int = ..., *, null: Literal[True],  **kwargs: Any) -> URLField[str | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, max_length: int = ..., *, null: Literal[False] = ..., **kwargs: Any) -> URLField[str]: ...
    @overload
    def __new__(cls, max_length: int = ..., *, null: bool = ..., **kwargs: Any) -> URLField[str]: ...

class IPAddressField[T: str | None](Field[T]):
    @overload
    def __new__(cls, *, null: Literal[True],  **kwargs: Any) -> IPAddressField[str | None]: ...  # pyright: ignore[reportOverlappingOverload]
    @overload
    def __new__(cls, *, null: Literal[False] = ..., **kwargs: Any) -> IPAddressField[str]: ...
    @overload
    def __new__(cls, *, null: bool = ..., **kwargs: Any) -> IPAddressField[str]: ...
    def __init__(self, *, null: bool = False, db_column: str | None = None) -> None: ...

class GenericIPAddressField[T: str | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        protocol: Literal["both", "IPv4", "IPv6"] = ...,
        unpack_ipv4: bool = ...,
        db_column: str | None = ...,
    ) -> GenericIPAddressField[str | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        protocol: Literal["both", "IPv4", "IPv6"] = ...,
        unpack_ipv4: bool = ...,
        db_column: str | None = ...,
    ) -> GenericIPAddressField[str]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        protocol: Literal["both", "IPv4", "IPv6"] = ...,
        unpack_ipv4: bool = ...,
        db_column: str | None = ...,
    ) -> GenericIPAddressField[str]: ...
    def __init__(
        self,
        *,
        protocol: Literal["both", "IPv4", "IPv6"] = "both",
        unpack_ipv4: bool = False,
        null: bool = False,
        db_column: str | None = None,
    ) -> None: ...

# ── Boolean ───────────────────────────────────────────────────────────────

class BooleanField[T: bool | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        default: bool | None = ...,
        db_column: str | None = ...,
    ) -> BooleanField[bool | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: bool | None = ...,
        db_column: str | None = ...,
    ) -> BooleanField[bool]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: bool | None = ...,
        db_column: str | None = ...,
    ) -> BooleanField[bool]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: bool | None = None,
        db_column: str | None = None,
    ) -> None: ...

# ── Date / Time ───────────────────────────────────────────────────────────

class DateField[T: datetime.date | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        auto_now: bool = ...,
        auto_now_add: bool = ...,
        default: datetime.date | None = ...,
        db_column: str | None = ...,
    ) -> DateField[datetime.date | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        auto_now: bool = ...,
        auto_now_add: bool = ...,
        default: datetime.date | None = ...,
        db_column: str | None = ...,
    ) -> DateField[datetime.date]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        auto_now: bool = ...,
        auto_now_add: bool = ...,
        default: datetime.date | None = ...,
        db_column: str | None = ...,
    ) -> DateField[datetime.date]: ...
    def __init__(
        self,
        *,
        auto_now: bool = False,
        auto_now_add: bool = False,
        null: bool = False,
        default: datetime.date | None = None,
        db_column: str | None = None,
    ) -> None: ...


class TimeField[T: datetime.time | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        default: datetime.time | None = ...,
        db_column: str | None = ...,
    ) -> TimeField[datetime.time | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: datetime.time | None = ...,
        db_column: str | None = ...,
    ) -> TimeField[datetime.time]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: datetime.time | None = ...,
        db_column: str | None = ...,
    ) -> TimeField[datetime.time]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: datetime.time | None = None,
        db_column: str | None = None,
    ) -> None: ...


class DateTimeField[T: datetime.datetime | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        auto_now: bool = ...,
        auto_now_add: bool = ...,
        default: datetime.datetime | None = ...,
        db_column: str | None = ...,
    ) -> DateTimeField[datetime.datetime | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        auto_now: bool = ...,
        auto_now_add: bool = ...,
        default: datetime.datetime | None = ...,
        db_column: str | None = ...,
    ) -> DateTimeField[datetime.datetime]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        auto_now: bool = ...,
        auto_now_add: bool = ...,
        default: datetime.datetime | None = ...,
        db_column: str | None = ...,
    ) -> DateTimeField[datetime.datetime]: ...
    def __init__(
        self,
        *,
        auto_now: bool = False,
        auto_now_add: bool = False,
        null: bool = False,
        default: datetime.datetime | None = None,
        db_column: str | None = None,
    ) -> None: ...


class DurationField[T: datetime.timedelta | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        default: datetime.timedelta | None = ...,
        db_column: str | None = ...,
    ) -> DurationField[datetime.timedelta | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: datetime.timedelta | None = ...,
        db_column: str | None = ...,
    ) -> DurationField[datetime.timedelta]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: datetime.timedelta | None = ...,
        db_column: str | None = ...,
    ) -> DurationField[datetime.timedelta]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: datetime.timedelta | None = None,
        db_column: str | None = None,
    ) -> None: ...

# ── Special types ─────────────────────────────────────────────────────────

class UUIDField[T: uuid.UUID | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        primary_key: bool = ...,
        default: uuid.UUID | None = ...,
        db_column: str | None = ...,
    ) -> UUIDField[uuid.UUID | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        primary_key: bool = ...,
        default: uuid.UUID | None = ...,
        db_column: str | None = ...,
    ) -> UUIDField[uuid.UUID]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        primary_key: bool = ...,
        default: uuid.UUID | None = ...,
        db_column: str | None = ...,
    ) -> UUIDField[uuid.UUID]: ...
    def __init__(
        self,
        *,
        primary_key: bool = False,
        default: uuid.UUID | None = None,
        null: bool = False,
        db_column: str | None = None,
    ) -> None: ...


class JSONField[T](Field[T]):
    """
    JSONField uses Any as default T because the deserialized value can be any
    JSON-representable type (dict, list, str, int, float, bool, None).
    Users can narrow: ``data: dict[str, int] = JSONField()``
    """
    @overload
    def __new__(
        cls,
        *,
        null: Literal[True],
        default: Any = ...,
        db_column: str | None = ...,
    ) -> JSONField[Any]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        default: Any = ...,
        db_column: str | None = ...,
    ) -> JSONField[Any]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        default: Any = ...,
        db_column: str | None = ...,
    ) -> JSONField[Any]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        default: Any = None,
        db_column: str | None = None,
    ) -> None: ...


class BinaryField[T: bytes | None](Field[T]):
    @overload
    def __new__(  # type: ignore[overload-overlap]
        cls,
        *,
        null: Literal[True],
        db_column: str | None = ...,
    ) -> BinaryField[bytes | None]: ...
    @overload
    def __new__(
        cls,
        *,
        null: Literal[False] = ...,
        db_column: str | None = ...,
    ) -> BinaryField[bytes]: ...
    @overload
    def __new__(
        cls,
        *,
        null: bool = ...,
        db_column: str | None = ...,
    ) -> BinaryField[bytes]: ...
    def __init__(
        self,
        *,
        null: bool = False,
        db_column: str | None = None,
    ) -> None: ...

# ── Relational fields ─────────────────────────────────────────────────────
#
# ForeignKey[M] stores the related model type in T so that forward-access
# descriptors (to be added in a future phase) can be typed correctly.
# For now the getter returns M | None (since FK values can be None before save)
# when null=True, and M otherwise.
#
# Because related model type (M) and nullability (T) are independent, we use
# two type parameters: M is the related model class, T is the descriptor value.

class ForeignKey[M, T](Field[T]):
    @overload
    def __new__(
        cls,
        to: type[M],
        on_delete: OnDelete,
        *,
        null: Literal[True],
        related_name: str | None = ...,
        to_field: str | None = ...,
        db_constraint: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> ForeignKey[M, M | None]: ...
    @overload
    def __new__(
        cls,
        to: type[M],
        on_delete: OnDelete,
        *,
        null: Literal[False] = ...,
        related_name: str | None = ...,
        to_field: str | None = ...,
        db_constraint: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> ForeignKey[M, M]: ...
    @overload
    def __new__(
        cls,
        to: type[M],
        on_delete: OnDelete,
        *,
        null: bool = ...,
        related_name: str | None = ...,
        to_field: str | None = ...,
        db_constraint: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> ForeignKey[M, M]: ...
    # String-form forward reference overloads
    @overload
    def __new__(
        cls,
        to: str,
        on_delete: OnDelete,
        *,
        null: Literal[True],
        related_name: str | None = ...,
        to_field: str | None = ...,
        db_constraint: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> ForeignKey[Any, Any]: ...
    @overload
    def __new__(
        cls,
        to: str,
        on_delete: OnDelete,
        *,
        null: bool = ...,
        related_name: str | None = ...,
        to_field: str | None = ...,
        db_constraint: bool = ...,
        db_index: bool = ...,
        db_column: str | None = ...,
    ) -> ForeignKey[Any, Any]: ...
    def __init__(
        self,
        to: type | str,
        on_delete: OnDelete,
        *,
        related_name: str | None = None,
        to_field: str | None = None,
        db_constraint: bool = True,
        null: bool = False,
        db_index: bool = True,
        db_column: str | None = None,
    ) -> None: ...


class OneToOneField[M, T](ForeignKey[M, T]):
    @overload
    def __new__(
        cls,
        to: type[M],
        on_delete: OnDelete,
        *,
        null: Literal[True],
        **kwargs: Any,
    ) -> OneToOneField[M, M | None]: ...
    @overload
    def __new__(
        cls,
        to: type[M],
        on_delete: OnDelete,
        *,
        null: Literal[False] = ...,
        **kwargs: Any,
    ) -> OneToOneField[M, M]: ...
    @overload
    def __new__(
        cls,
        to: type[M] | str,
        on_delete: OnDelete,
        *,
        null: bool = ...,
        **kwargs: Any,
    ) -> OneToOneField[Any, Any]: ...


class ManyToManyField[M](Field[Any]):
    """
    ManyToManyField returns the related manager on access, not a plain Python
    value.  The descriptor type stays ``Any`` for now; a future phase will add
    a ``RelatedManager[M]`` type.
    """
    def __init__(
        self,
        to: type[M] | str,
        *,
        through: type | str | None = None,
        db_table: str | None = None,
        related_name: str | None = None,
        symmetrical: bool | None = None,
    ) -> None: ...
