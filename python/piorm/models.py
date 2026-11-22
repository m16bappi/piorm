"""
Python-level Model base class.

Wraps the Rust `_core.Model` (which provides the Rust-side struct) and
adds Python-friendly `__init__`, `__init_subclass__`, `__getattr__`,
`__setattr__`, and `DoesNotExist`.

Registration into the global `ModelRegistry` is done by the Rust function
`_register_model_class`, called explicitly from `__init_subclass__`.

Note: pickling model instances is not supported. Use .values() queries
or serialize via your own __getstate__ / __setstate__ if needed.
"""

__all__ = ["Model"]

from typing import Any, ClassVar

from piorm._core import Model as _BaseModel
from piorm._core import _register_model_class
from piorm.exceptions import DoesNotExist, MultipleObjectsReturned


class _ModelMeta(type):
    """Metaclass that auto-injects ``__slots__ = ()`` into concrete model subclasses.

    When a user writes ``class Article(Model): ...`` without declaring ``__slots__``,
    Python would normally add ``__dict__`` to the class.  This metaclass intercepts
    class creation and adds ``__slots__ = ()`` to the namespace so instances stay
    slot-only.  Users who need dynamic attributes can opt back in by explicitly
    declaring ``__slots__ = ("__dict__",)`` in their subclass.
    """

    def __new__(
        mcs,
        name: str,
        bases: tuple[type, ...],
        namespace: dict[str, Any],
        **kwargs: Any,
    ) -> "_ModelMeta":
        if "__slots__" not in namespace:
            namespace["__slots__"] = ()
        return super().__new__(mcs, name, bases, namespace, **kwargs)


class Model(_BaseModel, metaclass=_ModelMeta):
    """Base class for all piorm model definitions.

    Subclass this to define a database model::

        from piorm.models import Model
        from piorm.fields import CharField, IntegerField

        class Article(Model):
            title = CharField(max_length=200)
            views = IntegerField(default=0)

    Each concrete subclass is automatically registered in the global
    ``ModelRegistry`` and given an ``objects`` manager.
    """

    # ── Slot layout ──────────────────────────────────────────────────────
    #
    # Five slots replace the old __dict__ + 3 dicts/sets:
    #
    #   _values   dict[str, Any]   field values (was _piorm_values)
    #   _dirty    int              bitmask of modified fields (was _piorm_dirty set)
    #   _adding   bool             True until first successful save() (was _piorm_state["adding"])
    #   _db       str | None       alias used for last save (was _piorm_state["db"])
    #   __weakref__                required by __slots__ for weakref support
    #
    # _field_bits is a ClassVar dict[str, int] set by _register_model_class (Task 14.1).
    # It maps field name → power-of-two bit used in the _dirty bitmask.
    __slots__ = ("_values", "_dirty", "_adding", "_db", "__weakref__")

    _field_bits: ClassVar[dict[str, int]]

    # ── Construction ─────────────────────────────────────────────────────

    def __init__(self, **kwargs: Any) -> None:
        self._values: dict[str, Any] = {}
        self._dirty: int = 0
        self._adding: bool = True
        self._db: str | None = None
        for k, v in kwargs.items():
            setattr(self, k, v)

    # ── Subclass registration ─────────────────────────────────────────────

    def __init_subclass__(cls, **kwargs: Any) -> None:
        super().__init_subclass__(**kwargs)

        # Skip abstract models — check only this class's own Meta, not inherited.
        own_meta = cls.__dict__.get("Meta", None)
        if own_meta and getattr(own_meta, "abstract", False):
            return

        # Register fields + build ModelDef in Rust, attach `objects` manager,
        # and set _field_bits on the class (Task 14.1).
        _register_model_class(cls)

        # Attach model-specific exception classes.
        cls.DoesNotExist = type(
            "DoesNotExist",
            (DoesNotExist,),
            {"__module__": cls.__module__},
        )
        cls.MultipleObjectsReturned = type(
            "MultipleObjectsReturned",
            (MultipleObjectsReturned,),
            {"__module__": cls.__module__},
        )

    # ── Descriptor protocol ───────────────────────────────────────────────

    def __getattr__(self, name: str) -> Any:
        """Fall through to _values for field reads.

        Called only when normal attribute lookup (slots + class dict) fails,
        i.e. only for field names that are not slots.
        """
        # Avoid infinite recursion if _values itself is not yet initialised
        # (can happen during __init__ before the first assignment).
        if name == "_values":
            raise AttributeError(name)
        try:
            return self._values[name]
        except KeyError:
            raise AttributeError(
                f"'{type(self).__name__}' object has no attribute '{name}'"
            ) from None

    def __setattr__(self, name: str, value: Any) -> None:
        """Route writes to slots or to _values + dirty bitmask.

        Slot names are handled by object.__setattr__ directly.
        Everything else is a field write: stored in _values and the
        corresponding bit is OR-ed into _dirty.
        """
        if name in ("_values", "_dirty", "_adding", "_db"):
            object.__setattr__(self, name, value)
            return

        # Field write — store value and mark field dirty.
        self._values[name] = value
        bit = self.__class__.__dict__.get("_field_bits", {}).get(name, 0)
        if bit:
            self._dirty |= bit

    # ── Properties ───────────────────────────────────────────────────────

    @property
    def pk(self) -> Any:
        """Return the primary-key value for the field that has primary_key=True."""
        pk_field = getattr(self.__class__, "_piorm_pk_field", "id")
        return self._values.get(pk_field)

    # ── Repr ──────────────────────────────────────────────────────────────

    def __repr__(self) -> str:
        pk_field = getattr(self.__class__, "_piorm_pk_field", "id")
        pk = self._values.get(pk_field, "?")
        return f"<{self.__class__.__name__}: pk={pk}>"

    # ── Abstract base ─────────────────────────────────────────────────────

    class Meta:
        abstract = True
