# python/ormpi/models.pyi
# Python 3.12+ stubs for the ormpi Model base class.

from typing import Any, ClassVar, Self, dataclass_transform
from ormpi._core import Manager, QuerySet
from ormpi.exceptions import DoesNotExist as _DoesNotExist
from ormpi.fields import Field

# ── ModelOptions ──────────────────────────────────────────────────────────

class ModelOptions:
    """Mirrors the ``class Meta`` inner class of a model."""
    db_table:        str
    ordering:        list[str]
    unique_together: list[list[str]]
    managed:         bool
    abstract:        bool

# ── Model ─────────────────────────────────────────────────────────────────

@dataclass_transform(
    eq_default=True,
    kw_only_default=True,
    field_specifiers=(Field,),
)
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
    _ormpi_values: dict[str, Any]
    _ormpi_dirty:  set[str]
    _ormpi_state:  dict[str, Any]

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

    async def delete(self, using: str = "default") -> None: ...

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
    class Meta:
        abstract:        ClassVar[bool]
        db_table:        ClassVar[str]
        ordering:        ClassVar[list[str]]
        unique_together: ClassVar[list[list[str]]]
        managed:         ClassVar[bool]
