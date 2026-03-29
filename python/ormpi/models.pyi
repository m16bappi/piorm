# python/ormpi/models.pyi
# Python 3.12+ stubs for the ormpi Model base class.

from typing import Any, ClassVar, Self
from ormpi._core import Manager
from ormpi.exceptions import DoesNotExist as _DoesNotExist

# ── ModelOptions ──────────────────────────────────────────────────────────

class ModelOptions:
    """Mirrors the ``class Meta`` inner class of a model."""
    db_table:        str
    ordering:        list[str]
    unique_together: list[list[str]]
    managed:         bool
    abstract:        bool

# ── Model ─────────────────────────────────────────────────────────────────
#
# Note: @dataclass_transform is intentionally NOT used here.
# pyright/Pylance require explicit type annotations (name: str = CharField(...))
# to synthesize __init__ parameters via @dataclass_transform. Without them,
# it generates an empty __init__ and reports "No parameter named 'name'".
# Instead, __init__(self, **kwargs: Any) accepts all field kwargs and lets
# the descriptor protocol (Field[T].__get__) handle instance attribute typing.
#
class Model:
    """Base class for all ormpi models.

    Subclass to define a table::

        class Author(Model):
            name  = CharField(max_length=100)
            email = CharField(max_length=200, unique=True)
            bio   = TextField(null=True)

    Type checkers that understand ``@dataclass_transform`` synthesise a typed
    ``__init__`` from the declared fields::

        Author(name="Alice", email="a@example.com", bio=None)
        # (name: str, email: str, bio: str | None)

    ``objects`` is resolved per-subclass via ``Self``::

        Author.objects           # Manager[Author]
        Author.objects.get(pk=1) # Coroutine[Any, Any, Author]
    """

    # ── Private ORM state ──────────────────────────────────────────────
    __slots__:   ClassVar[tuple[str, ...]]
    _field_bits: ClassVar[dict[str, int]]
    _values:  dict[str, Any]
    _dirty:   int
    _adding:  bool
    _db:      str | None

    # ── Class-level attributes ─────────────────────────────────────────
    #
    # ``objects`` typed as ``Manager[Self]`` so that:
    #   Author.objects.get(pk=1)   →  Coroutine[Any, Any, Author]
    #   Author.objects.filter(...) →  QuerySet[Author]
    #   Author.objects.first()     →  Coroutine[Any, Any, Author | None]
    #
    # Pyright resolves Self correctly when accessed on a concrete subclass.
    # Mypy note: mypy < 1.12 has partial Self-in-ClassVar support. If mypy
    # strict mode rejects ``Manager[Self]`` here, use:
    #   objects: ClassVar[Manager[Any]]
    # as a temporary workaround — pyright will still infer correctly.
    objects:      ClassVar[Manager[Self]]
    DoesNotExist: ClassVar[type[_DoesNotExist]]
    _meta:        ClassVar[ModelOptions]

    # ── Primary key ────────────────────────────────────────────────────
    @property
    def pk(self) -> Any:
        """Shorthand for the primary-key field value.

        The type is ``Any`` because the PK type depends on which field the
        user declared (or the implicit BIGINT if none).  Annotate your model's
        ``id`` field explicitly for a narrower type.
        """
        ...

    # ── Constructor ────────────────────────────────────────────────────
    #
    # ``@dataclass_transform`` synthesises a typed __init__ per subclass.
    # The fallback below is used for Model itself (not concrete subclasses).
    def __init__(self, **kwargs: Any) -> None: ...

    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

    # ── Persistence ────────────────────────────────────────────────────
    async def save(
        self,
        update_fields: list[str] | None = None,
        using: str = "default",
    ) -> None: ...

    async def delete(self, using: str = "default") -> tuple[int, dict[str, int]]: ...

    async def refresh_from_db(
        self,
        fields: list[str] | None = None,
        using: str = "default",
    ) -> None: ...

    # ── Class methods ──────────────────────────────────────────────────
    @classmethod
    def from_db(
        cls,
        db: str,
        field_names: list[str],
        values: list[Any],
    ) -> Self:
        """Reconstruct an instance from a database row.

        Returns ``Self`` so that ``Author.from_db(...)`` is typed as ``Author``,
        not as the base ``Model``.
        """
        ...

    # ── Inner Meta ─────────────────────────────────────────────────────
    #
    # Declared as ``ClassVar[type]`` (not a nested class) so that
    # subclasses can freely define their own ``class Meta:`` without
    # triggering ``reportIncompatibleVariableOverride``.  Recognised
    # Meta options (db_table, ordering, …) are validated at runtime;
    # typed access is available via the ``_meta: ModelOptions`` attribute.
    Meta: ClassVar[type]
