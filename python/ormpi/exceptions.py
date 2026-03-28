__all__ = [
    "OrmpiError",
    "DatabaseError",
    "IntegrityError",
    "OperationalError",
    "ValidationError",
    "FieldError",
    "TransactionError",
    "MigrationError",
    "DoesNotExist",
    "MultipleObjectsReturned",
    "ImproperlyConfigured",
]


class OrmpiError(Exception):
    pass


class DatabaseError(OrmpiError):
    pass


class IntegrityError(DatabaseError):
    pass


class OperationalError(DatabaseError):
    pass


class ValidationError(OrmpiError):
    def __init__(self, message: str) -> None:
        self.message = message
        super().__init__(message)


class FieldError(OrmpiError):
    pass


class TransactionError(OrmpiError):
    pass


class MigrationError(OrmpiError):
    pass


class DoesNotExist(OrmpiError):
    pass


class MultipleObjectsReturned(OrmpiError):
    pass


class ImproperlyConfigured(OrmpiError):
    pass
