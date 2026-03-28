"""
Query expression objects: Q combinators, F references, and aggregate functions.

    from ormpi.expr import Q, F, Count, Sum, Avg, Max, Min
    from ormpi import expr   # namespace access: expr.Q(...)
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

try:
    from ormpi._core import Q, Aggregate, Count, Sum, Avg, Max, Min  # type: ignore[attr-defined]
except ImportError:
    # Pure-Python fallback used during stub-only / CI type-check runs
    class Q:  # type: ignore[no-redef]
        def __init__(self, **kwargs):  ...
        def __and__(self, other): return other
        def __or__(self, other): return other
        def __invert__(self): return self

    class Aggregate:  # type: ignore[no-redef]
        def __init__(self, field: str, **kwargs): ...

    class Count(Aggregate): ...   # type: ignore[no-redef]
    class Sum(Aggregate): ...     # type: ignore[no-redef]
    class Avg(Aggregate): ...     # type: ignore[no-redef]
    class Max(Aggregate): ...     # type: ignore[no-redef]
    class Min(Aggregate): ...     # type: ignore[no-redef]


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
