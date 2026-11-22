# pyright: ignore[reportAssignmentType]
"""
Field types for piorm model definitions.

These are imported from the Rust `_core` extension. This module re-exports
them for convenience and provides a stable import path.
"""
from typing import Any

__all__ = [
    "Field",
    "IntegerField",
    "BigIntegerField",
    "SmallIntegerField",
    "PositiveIntegerField",
    "PositiveBigIntegerField",
    "PositiveSmallIntegerField",
    "FloatField",
    "DecimalField",
    "CharField",
    "TextField",
    "SlugField",
    "EmailField",
    "URLField",
    "IPAddressField",
    "GenericIPAddressField",
    "BooleanField",
    "DateField",
    "TimeField",
    "DateTimeField",
    "DurationField",
    "UUIDField",
    "JSONField",
    "BinaryField",
    "ForeignKey",
    "OneToOneField",
    "ManyToManyField",
    "CASCADE",
    "PROTECT",
    "SET_NULL",
    "SET_DEFAULT",
    "DO_NOTHING",
    "RESTRICT",
]

try:
    from piorm._core import (  # type: ignore[attr-defined]  # pyright: ignore[reportAttributeAccessIssue,reportAssignmentType]
        BigIntegerField,
        BinaryField,
        BooleanField,
        CASCADE,
        CharField,
        DateField,
        DateTimeField,
        DecimalField,
        DurationField,
        EmailField,
        FloatField,
        ForeignKey,
        GenericIPAddressField,
        IntegerField,
        IPAddressField,
        JSONField,
        ManyToManyField,
        OneToOneField,
        PositiveBigIntegerField,
        PositiveIntegerField,
        PositiveSmallIntegerField,
        PROTECT,
        SET_NULL,
        SlugField,
        SmallIntegerField,
        TextField,
        TimeField,
        UUIDField,
    )
    from piorm._core import UrlField as URLField  # type: ignore[attr-defined]  # pyright: ignore

    # These may not be in all build variants; fall back to shims if absent.
    try:
        from piorm._core import Field  # type: ignore[attr-defined]  # pyright: ignore
    except ImportError:
        class Field:  # type: ignore[no-redef]
            name: str = ""
            attname: str = ""
            null: bool = False
            unique: bool = False
            primary_key: bool = False
            db_index: bool = False
            editable: bool = True

            def __init__(self, **kwargs: Any) -> None:
                for k, v in kwargs.items():
                    setattr(self, k, v)

            def contribute_to_class(self, cls: type, name: str) -> None:
                self.name = name

    try:
        from piorm._core import SET_DEFAULT  # type: ignore[attr-defined]  # pyright: ignore
    except ImportError:
        SET_DEFAULT: str = "SET_DEFAULT"  # type: ignore[misc]

    try:
        from piorm._core import DO_NOTHING  # type: ignore[attr-defined]  # pyright: ignore
    except ImportError:
        DO_NOTHING: str = "DO_NOTHING"  # type: ignore[misc]

    try:
        from piorm._core import RESTRICT  # type: ignore[attr-defined]  # pyright: ignore
    except ImportError:
        RESTRICT: str = "RESTRICT"  # type: ignore[misc]

except ImportError:
    # Fallback pure-Python shims for environments without the compiled extension.
    class Field:  # type: ignore[no-redef]
        name: str = ""
        attname: str = ""
        null: bool = False
        unique: bool = False
        primary_key: bool = False
        db_index: bool = False
        editable: bool = True

        def __init__(self, **kwargs: Any) -> None:
            for k, v in kwargs.items():
                setattr(self, k, v)

        def contribute_to_class(self, cls: type, name: str) -> None:
            self.name = name

    class IntegerField(Field): ...  # type: ignore[no-redef]
    class BigIntegerField(Field): ...  # type: ignore[no-redef]
    class SmallIntegerField(IntegerField): ...  # type: ignore[no-redef]
    class PositiveIntegerField(IntegerField): ...  # type: ignore[no-redef]
    class PositiveBigIntegerField(IntegerField): ...  # type: ignore[no-redef]
    class PositiveSmallIntegerField(IntegerField): ...  # type: ignore[no-redef]
    class FloatField(Field): ...  # type: ignore[no-redef]
    class DecimalField(Field): ...  # type: ignore[no-redef]
    class CharField(Field): ...  # type: ignore[no-redef]
    class TextField(Field): ...  # type: ignore[no-redef]
    class SlugField(CharField): ...  # type: ignore[no-redef]
    class EmailField(CharField): ...  # type: ignore[no-redef]
    class URLField(CharField): ...  # type: ignore[no-redef]
    class IPAddressField(Field): ...  # type: ignore[no-redef]
    class GenericIPAddressField(Field): ...  # type: ignore[no-redef]
    class BooleanField(Field): ...  # type: ignore[no-redef]
    class DateField(Field): ...  # type: ignore[no-redef]
    class TimeField(Field): ...  # type: ignore[no-redef]
    class DateTimeField(Field): ...  # type: ignore[no-redef]
    class DurationField(Field): ...  # type: ignore[no-redef]
    class UUIDField(Field): ...  # type: ignore[no-redef]
    class JSONField(Field): ...  # type: ignore[no-redef]
    class BinaryField(Field): ...  # type: ignore[no-redef]
    class ForeignKey(Field): ...  # type: ignore[no-redef]
    class OneToOneField(ForeignKey): ...  # type: ignore[no-redef]
    class ManyToManyField(Field): ...  # type: ignore[no-redef]
    CASCADE: str = "CASCADE"
    PROTECT: str = "PROTECT"
    SET_NULL: str = "SET_NULL"
    SET_DEFAULT: str = "SET_DEFAULT"
    DO_NOTHING: str = "DO_NOTHING"
    RESTRICT: str = "RESTRICT"
