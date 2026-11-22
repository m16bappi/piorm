"""
piorm — async Python ORM built on Rust (sea-query + sqlx).

Canonical import patterns:

    from piorm import configure, close, Model
    from piorm.fields import CharField, IntegerField, ForeignKey, CASCADE
    from piorm.expr import Q, F, Count, Sum
    from piorm.db import connections
    from piorm.exceptions import DoesNotExist, ValidationError
"""

# ── Core setup ────────────────────────────────────────────────────────────
from piorm._core import close, execute_raw, get_version

# ── Sub-modules ────────────────────────────────────────────────────────────
import piorm.db as db
import piorm.exceptions as exceptions
import piorm.expr as expr
import piorm.fields as fields
from piorm.db import configure

# ── Model ─────────────────────────────────────────────────────────────────
from piorm.models import Model

# ── Public typing helpers ──────────────────────────────────────────────────
from piorm._core import Manager, QuerySet
from piorm.fields import Field

__version__: str = get_version()

__all__ = [
    # setup
    "configure",
    "close",
    "execute_raw",
    "__version__",
    # model
    "Model",
    # typing helpers
    "Manager",
    "QuerySet",
    "Field",
    # sub-modules
    "fields",
    "expr",
    "db",
    "exceptions",
]
