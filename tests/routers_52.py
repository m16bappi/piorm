"""Test routers used by test_phase5_2_routing.py via dotted-path loading."""


class AlwaysDefaultRouter:
    """Returns None for all queries → defers to 'default'."""

    def db_for_read(self, model, **hints):
        return None

    def db_for_write(self, model, **hints):
        return None

    def allow_migrate(self, db, app_label, model_name=None, **hints):
        return None
