from typing import Any, ClassVar, Literal
from collections.abc import Sequence
import datetime
import decimal
import uuid

# ── on_delete constants ───────────────────────────────────────────────────

type OnDelete = Literal["CASCADE", "PROTECT", "SET_NULL", "SET_DEFAULT", "DO_NOTHING", "RESTRICT"]

CASCADE:    OnDelete
PROTECT:    OnDelete
SET_NULL:   OnDelete
SET_DEFAULT: OnDelete
DO_NOTHING: OnDelete
RESTRICT:   OnDelete

# ── choices type alias ────────────────────────────────────────────────────

type Choices[T] = Sequence[tuple[T, str]]

# ── base Field ────────────────────────────────────────────────────────────

class Field:
    """Base descriptor for all ormpi field types."""
    name: str          # Python attribute name on the model class
    attname: str       # Column-level attribute name (same as name for scalars)
    null: bool
    unique: bool
    primary_key: bool
    db_index: bool
    editable: bool
    column: str        # Actual SQL column name (respects db_column override)

    def contribute_to_class(self, cls: type, name: str) -> None: ...
    def __get__(self, obj: Any, objtype: type | None = None) -> Any: ...
    def __set__(self, obj: Any, value: Any) -> None: ...
    def db_column(self) -> str: ...

# ── integer fields ────────────────────────────────────────────────────────

class IntegerField(Field):
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

class BigIntegerField(Field):
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

class SmallIntegerField(IntegerField): ...
class PositiveIntegerField(IntegerField): ...
class PositiveBigIntegerField(BigIntegerField): ...
class PositiveSmallIntegerField(SmallIntegerField): ...

# ── floating-point / decimal ───────────────────────────────────────────────

class FloatField(Field):
    def __init__(
        self,
        *,
        null: bool = False,
        default: float | None = None,
        unique: bool = False,
        db_index: bool = False,
        db_column: str | None = None,
    ) -> None: ...

class DecimalField(Field):
    max_digits: int
    decimal_places: int
    def __init__(
        self,
        max_digits: int,
        decimal_places: int,
        *,
        null: bool = False,
        default: decimal.Decimal | str | None = None,
        db_column: str | None = None,
    ) -> None: ...

# ── string fields ────────────────────────────────────────────────────────

class CharField(Field):
    max_length: int
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

class TextField(Field):
    def __init__(
        self,
        *,
        null: bool = False,
        default: str | None = None,
        db_column: str | None = None,
    ) -> None: ...

class SlugField(CharField): ...
class EmailField(CharField): ...
class URLField(CharField): ...
class IPAddressField(Field): ...
class GenericIPAddressField(Field):
    def __init__(
        self,
        *,
        protocol: Literal["both", "IPv4", "IPv6"] = "both",
        unpack_ipv4: bool = False,
        null: bool = False,
        db_column: str | None = None,
    ) -> None: ...

# ── boolean ───────────────────────────────────────────────────────────────

class BooleanField(Field):
    def __init__(
        self,
        *,
        null: bool = False,
        default: bool | None = None,
        db_column: str | None = None,
    ) -> None: ...

# ── date / time ───────────────────────────────────────────────────────────

class DateField(Field):
    def __init__(
        self,
        *,
        auto_now: bool = False,
        auto_now_add: bool = False,
        null: bool = False,
        default: datetime.date | None = None,
        db_column: str | None = None,
    ) -> None: ...

class TimeField(Field):
    def __init__(
        self,
        *,
        null: bool = False,
        default: datetime.time | None = None,
        db_column: str | None = None,
    ) -> None: ...

class DateTimeField(Field):
    def __init__(
        self,
        *,
        auto_now: bool = False,
        auto_now_add: bool = False,
        null: bool = False,
        default: datetime.datetime | None = None,
        db_column: str | None = None,
    ) -> None: ...

class DurationField(Field):
    def __init__(
        self,
        *,
        null: bool = False,
        default: datetime.timedelta | None = None,
        db_column: str | None = None,
    ) -> None: ...

# ── special types ────────────────────────────────────────────────────────

class UUIDField(Field):
    def __init__(
        self,
        *,
        primary_key: bool = False,
        default: uuid.UUID | None = None,
        null: bool = False,
        db_column: str | None = None,
    ) -> None: ...

class JSONField(Field):
    def __init__(
        self,
        *,
        null: bool = False,
        default: Any = None,
        db_column: str | None = None,
    ) -> None: ...

class BinaryField(Field):
    def __init__(
        self,
        *,
        null: bool = False,
        db_column: str | None = None,
    ) -> None: ...

# ── relational fields ────────────────────────────────────────────────────

class ForeignKey(Field):
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

class OneToOneField(ForeignKey): ...

class ManyToManyField(Field):
    def __init__(
        self,
        to: type | str,
        *,
        through: type | str | None = None,
        db_table: str | None = None,
        related_name: str | None = None,
        symmetrical: bool | None = None,
    ) -> None: ...
