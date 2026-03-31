"""
Exception hierarchy for ormpi.

All exceptions are also accessible via `ormpi.exceptions.*`.
"""

__all__ = [
    "OrmpiError",
    "DatabaseError",
    "IntegrityError",
    "OperationalError",
    "ValidationError",
    "FieldError",
    "MigrationError",
    "DoesNotExist",
    "MultipleObjectsReturned",
    "ImproperlyConfigured",
]


class OrmpiError(Exception):
    """Base class for all ormpi exceptions."""


class DatabaseError(OrmpiError):
    """A database-level error occurred."""


class IntegrityError(DatabaseError):
    """A constraint (UNIQUE, FK, NOT NULL) was violated."""


class OperationalError(DatabaseError):
    """The database reported an operational error (connection lost, syntax, etc.)."""


class ValidationError(OrmpiError):
    """A field value failed validation."""
    message: str

    def __init__(self, message: str) -> None:
        self.message = message
        super().__init__(message)


class FieldError(OrmpiError):
    """An invalid field name or lookup was used."""


class MigrationError(OrmpiError):
    """Migration detection, generation, or application failed."""


class DoesNotExist(OrmpiError):
    """Raised when `.get()` matches no rows."""


class MultipleObjectsReturned(OrmpiError):
    """Raised when `.get()` matches more than one row."""


class ImproperlyConfigured(OrmpiError):
    """ormpi was not configured correctly (e.g., configure() not called)."""
