"""
Query expression objects: Q combinators, F references, and aggregate functions.

    from piorm.expr import Q, F, Count, Sum, Avg, Max, Min
    from piorm import expr   # namespace access: expr.Q(...)
"""

__all__ = [
    "Q",
    "F",
    "Aggregate",
    "Count",
    "Sum",
    "Avg",
    "Max",
    "Min",
]

from piorm._core import Q, Count, Sum, Avg, Max, Min  # type: ignore[attr-defined]


class Aggregate:  # type: ignore[no-redef]
    """Base class for aggregate functions."""
    def __init__(self, field: str, **kwargs): ...


class F:
    """Reference to a model field for use in annotations and updates.

    Example::

        # Increment a counter atomically
        await Post.objects.filter(pk=1).update(views=F("views") + 1)
    """

    def __init__(self, name: str) -> None:
        self.name = name

    def __repr__(self) -> str:
        return f"F({self.name!r})"

    # Arithmetic operators — return an F-expression node
    def __add__(self, other):      return _FExpr(self, "+", other)
    def __radd__(self, other):     return _FExpr(other, "+", self)
    def __sub__(self, other):      return _FExpr(self, "-", other)
    def __rsub__(self, other):     return _FExpr(other, "-", self)
    def __mul__(self, other):      return _FExpr(self, "*", other)
    def __rmul__(self, other):     return _FExpr(other, "*", self)
    def __truediv__(self, other):  return _FExpr(self, "/", other)
    def __mod__(self, other):      return _FExpr(self, "%", other)
    def __neg__(self):             return _FExpr(None, "-", self)


class _FExpr:
    """Internal: represents a binary expression built from F() objects."""
    __slots__ = ("lhs", "op", "rhs")

    def __init__(self, lhs, op: str, rhs) -> None:
        self.lhs = lhs
        self.op  = op
        self.rhs = rhs

    def __repr__(self) -> str:
        return f"({self.lhs!r} {self.op} {self.rhs!r})"

    # Allow chaining: F("x") + 1 + 2
    def __add__(self, other):  return _FExpr(self, "+", other)
    def __sub__(self, other):  return _FExpr(self, "-", other)
    def __mul__(self, other):  return _FExpr(self, "*", other)
