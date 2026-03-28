"""
ormpi — async Python ORM built on Rust (sea-query + sqlx).

Canonical import patterns:

    from ormpi import configure, close, Model
    from ormpi.fields import CharField, IntegerField, ForeignKey, CASCADE
    from ormpi.expr import Q, F, Count, Sum
    from ormpi.transaction import atomic, begin, Transaction
    from ormpi.db import connections
    from ormpi.exceptions import DoesNotExist, ValidationError
"""

# ── Sub-modules ────────────────────────────────────────────────────────────
import ormpi.db as db
import ormpi.exceptions as exceptions
import ormpi.expr as expr
import ormpi.fields as fields
import ormpi.transaction as transaction

# ── Core setup ────────────────────────────────────────────────────────────
from ormpi._core import close, execute_raw, get_version
from ormpi.db import configure

# ── Model ─────────────────────────────────────────────────────────────────
from ormpi.models import Model

__version__: str = get_version()

__all__ = [
    # setup
    "configure",
    "close",
    "execute_raw",
    "__version__",
    # model
    "Model",
    # sub-modules
    "fields",
    "expr",
    "transaction",
    "db",
    "exceptions",
]
