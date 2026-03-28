# python/ormpi/expr.pyi
# Public stubs for ormpi query expressions.
# Python 3.12+ only — no "from __future__ import annotations".

from typing import Any

# ── Operand type alias ────────────────────────────────────────────────────
# Used to type __init__ of _FExpr and the attribute types.

type _Operand = F | _FExpr | int | float | str | bytes | None

# ── _FExpr — internal binary expression node ──────────────────────────────

class _FExpr:
    """Internal: a binary (or unary) arithmetic expression built from F objects.

    Not part of the public API — returned by F arithmetic operators.
    Supports further chaining: ``F("x") + 1 + 2``.
    """
    lhs: _Operand
    op:  str
    rhs: _Operand

    def __init__(self, lhs: _Operand, op: str, rhs: _Operand) -> None: ...
    def __repr__(self) -> str: ...

    # Forward operators (self op other)
    def __add__(self,      other: _Operand) -> _FExpr: ...
    def __sub__(self,      other: _Operand) -> _FExpr: ...
    def __mul__(self,      other: _Operand) -> _FExpr: ...
    def __truediv__(self,  other: _Operand) -> _FExpr: ...
    def __mod__(self,      other: _Operand) -> _FExpr: ...
    def __pow__(self,      other: _Operand) -> _FExpr: ...

    # Reverse operators (other op self)
    def __radd__(self,     other: _Operand) -> _FExpr: ...
    def __rsub__(self,     other: _Operand) -> _FExpr: ...
    def __rmul__(self,     other: _Operand) -> _FExpr: ...
    def __rtruediv__(self, other: _Operand) -> _FExpr: ...
    def __rmod__(self,     other: _Operand) -> _FExpr: ...

    # Unary
    def __neg__(self) -> _FExpr: ...
    def __pos__(self) -> _FExpr: ...
    def __abs__(self) -> _FExpr: ...

# ── F — field reference ───────────────────────────────────────────────────

class F:
    """Reference to a model field for use in expressions and updates.

    Examples::

        # Atomic increment
        await Post.objects.filter(pk=1).update(views=F("views") + 1)

        # Expression in annotation
        await Order.objects.annotate(profit=F("revenue") - F("cost"))

        # Negation
        await Score.objects.annotate(deficit=-(F("target") - F("actual")))
    """
    name: str

    def __init__(self, name: str) -> None: ...
    def __repr__(self) -> str: ...

    # Forward operators
    def __add__(self,      other: _Operand) -> _FExpr: ...
    def __sub__(self,      other: _Operand) -> _FExpr: ...
    def __mul__(self,      other: _Operand) -> _FExpr: ...
    def __truediv__(self,  other: _Operand) -> _FExpr: ...
    def __mod__(self,      other: _Operand) -> _FExpr: ...
    def __pow__(self,      other: _Operand) -> _FExpr: ...

    # Reverse operators (left operand is a plain number/string)
    def __radd__(self,     other: _Operand) -> _FExpr: ...
    def __rsub__(self,     other: _Operand) -> _FExpr: ...
    def __rmul__(self,     other: _Operand) -> _FExpr: ...
    def __rtruediv__(self, other: _Operand) -> _FExpr: ...
    def __rmod__(self,     other: _Operand) -> _FExpr: ...

    # Unary
    def __neg__(self) -> _FExpr: ...
    def __pos__(self) -> _FExpr: ...
    def __abs__(self) -> _FExpr: ...

# ── Q — filter combinator ─────────────────────────────────────────────────

class Q:
    """Filter condition combinator supporting ``&``, ``|``, ``~``.

    Examples::

        Q(name="Alice") | Q(name="Bob")
        ~Q(active=False)
        Q(age__gte=18) & Q(country="US")

    ``Q`` objects can be passed as positional arguments to ``filter``/``exclude``,
    or combined before passing::

        Author.objects.filter(Q(name="Alice") | Q(name="Bob"))
    """
    def __init__(self, **kwargs: Any) -> None: ...

    def __and__(self,    other: Q) -> Q: ...
    def __or__(self,     other: Q) -> Q: ...
    def __invert__(self)          -> Q: ...
    def __rand__(self,   other: Q) -> Q: ...
    def __ror__(self,    other: Q) -> Q: ...
    def __repr__(self)             -> str: ...

# ── Aggregate functions ───────────────────────────────────────────────────

class Aggregate:
    """Base class for all SQL aggregate functions.

    Args:
        field: The field name (string) or an ``F`` expression to aggregate.
               Examples: ``Count("id")``, ``Sum(F("revenue"))``.
        distinct: When ``True``, only count/sum distinct values.
        filter: An optional ``Q`` filter applied before aggregating.
        output_field: Optional field instance specifying the result type.
    """
    def __init__(
        self,
        field: str | F,
        *,
        distinct: bool = False,
        filter: Q | None = None,
        output_field: Any = None,
    ) -> None: ...

class Count(Aggregate):
    """SQL COUNT aggregate.  Returns an ``int``."""
    ...

class Sum(Aggregate):
    """SQL SUM aggregate."""
    ...

class Avg(Aggregate):
    """SQL AVG aggregate."""
    ...

class Max(Aggregate):
    """SQL MAX aggregate."""
    ...

class Min(Aggregate):
    """SQL MIN aggregate."""
    ...
