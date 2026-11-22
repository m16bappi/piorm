"""
Exception hierarchy for piorm.

All exceptions are also accessible via `piorm.exceptions.*`.
"""

__all__ = [
    "PiormError",
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


class PiormError(Exception):
    """Base class for all piorm exceptions."""


class DatabaseError(PiormError):
    """A database-level error occurred."""


class IntegrityError(DatabaseError):
    """A constraint (UNIQUE, FK, NOT NULL) was violated."""


class OperationalError(DatabaseError):
    """The database reported an operational error (connection lost, syntax, etc.)."""


class ValidationError(PiormError):
    """A field value failed validation."""
    message: str

    def __init__(self, message: str) -> None:
        self.message = message
        super().__init__(message)


class FieldError(PiormError):
    """An invalid field name or lookup was used."""


class MigrationError(PiormError):
    """Migration detection, generation, or application failed."""


class DoesNotExist(PiormError):
    """Raised when `.get()` matches no rows."""


class MultipleObjectsReturned(PiormError):
    """Raised when `.get()` matches more than one row."""


class ImproperlyConfigured(PiormError):
    """piorm was not configured correctly (e.g., configure() not called)."""
