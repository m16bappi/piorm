class OrmpiError(Exception):
    pass


class DoesNotExist(OrmpiError):
    pass


class MultipleObjectsReturned(OrmpiError):
    pass


class FieldError(OrmpiError):
    pass


class ValidationError(OrmpiError):
    pass
