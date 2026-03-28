"""
Python-level Model base class.

Wraps the Rust `_core.Model` (which provides the Rust-side struct) and
adds Python-friendly `__init__`, `__init_subclass__`, and `DoesNotExist`.
Registration into the global `ModelRegistry` is done by the Rust function
`_register_model_class`, called explicitly from `__init_subclass__`.
"""

from __future__ import annotations

__all__ = ["Model"]

from typing import Any

from ormpi._core import Model as _BaseModel
from ormpi._core import _register_model_class
from ormpi.exceptions import DoesNotExist


class Model(_BaseModel):
    """Base class for all ormpi model definitions.

    Subclass this to define a database model::

        from ormpi.models import Model
        from ormpi._core import CharField, IntegerField

        class Article(Model):
            title = CharField(max_length=200)
            views = IntegerField(default=0)

    Each concrete subclass is automatically registered in the global
    ``ModelRegistry`` and given an ``objects`` manager.
    """

    _ormpi_values: dict[str, Any]
    _ormpi_dirty: set[str]

    def __init__(self, **kwargs: Any) -> None:
        self._ormpi_values = {}
        self._ormpi_dirty = set()
        self._ormpi_state = {"adding": True, "db": None}
        for k, v in kwargs.items():
            setattr(self, k, v)

    def __init_subclass__(cls, **kwargs: Any) -> None:
        super().__init_subclass__(**kwargs)

        # Skip abstract models — check only this class's own Meta, not inherited.
        own_meta = cls.__dict__.get("Meta", None)
        if own_meta and getattr(own_meta, "abstract", False):
            return

        # Register fields + build ModelDef in Rust, attach `objects` manager.
        _register_model_class(cls)

        # Attach a model-specific DoesNotExist exception.
        cls.DoesNotExist = type(
            "DoesNotExist",
            (DoesNotExist,),
            {"__module__": cls.__module__},
        )

    @property
    def pk(self) -> Any:
        """Return the primary-key value (always stored under 'id' by default)."""
        return self._ormpi_values.get("id")

    def __repr__(self) -> str:
        pk = self._ormpi_values.get("id", "?")
        return f"<{self.__class__.__name__}: pk={pk}>"

    class Meta:
        abstract = True
