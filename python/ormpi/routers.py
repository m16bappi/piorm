"""
Database router protocol.

Subclass `DatabaseRouter` and return a database alias from `db_for_read` /
`db_for_write` to direct queries.  Return ``None`` to defer to the next router.
"""


class DatabaseRouter:
    """Abstract base class for database routers.

    All methods return ``None`` by default, deferring to the next router (or
    ultimately the "default" database).
    """

    def db_for_read(self, model, **hints):
        """Return the alias to use for reading ``model``, or ``None`` to defer."""
        return None

    def db_for_write(self, model, **hints):
        """Return the alias to use for writing ``model``, or ``None`` to defer."""
        return None

    def allow_relation(self, obj1, obj2, **hints):
        """Return ``True`` if a relation between ``obj1`` and ``obj2`` is allowed."""
        return None

    def allow_migrate(self, db, app_label, model_name=None, **hints):
        """Return ``True`` if migration is allowed on ``db`` for this model."""
        return None
