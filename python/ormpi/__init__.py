"""
ormpi — A Django-like async ORM built on Rust (sea-query + sqlx).
"""

from ormpi import db
from ormpi._core import Avg, Count, Max, Min, Q, Sum, close, configure, execute_raw, get_version
from ormpi.models import Model

__version__: str = get_version()
__all__ = [
    "configure",
    "close",
    "execute_raw",
    "__version__",
    "Model",
    "db",
    "Q",
    "Count",
    "Sum",
    "Avg",
    "Max",
    "Min",
]
