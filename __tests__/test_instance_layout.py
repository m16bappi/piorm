"""
Phase 14 — instance layout correctness tests.

These tests verify the __slots__ + bitmask dirty-tracking layout without
requiring a live database connection.
"""
import sys
import pytest
from piorm.models import Model
from piorm.fields import CharField, IntegerField


class LayoutArticle(Model):
    title = CharField(max_length=200)
    views = IntegerField(default=0)

    class Meta:
        # Use a fake db_table so no actual DB migration is needed
        db_table = "test_article_layout"


class TestSlots:
    def test_no_dict_on_instance(self):
        """Model instances must not have __dict__ after __slots__ migration."""
        a = LayoutArticle()
        assert not hasattr(a, "__dict__"), (
            "__dict__ present — __slots__ is not active. "
            "Check that PyModel has `frozen` and that models.py declares __slots__."
        )

    def test_slots_declared(self):
        assert "__slots__" in Model.__dict__
        assert set(Model.__slots__) == {"_values", "_dirty", "_adding", "_db", "__weakref__"}

    def test_field_bits_set_on_class(self):
        bits = LayoutArticle._field_bits
        assert "id"    in bits
        assert "title" in bits
        assert "views" in bits
        # All bits are distinct powers of two
        values = list(bits.values())
        assert len(values) == len(set(values))
        for v in values:
            assert v > 0 and (v & (v - 1)) == 0, f"{v} is not a power of two"

    def test_weakref_support(self):
        import weakref
        a = LayoutArticle()
        ref = weakref.ref(a)
        assert ref() is a


class TestDirtyTracking:
    def test_clean_after_construction_no_kwargs(self):
        a = LayoutArticle()
        assert a._dirty == 0

    def test_dirty_after_kwarg_init(self):
        a = LayoutArticle(title="Hello")
        assert a._dirty & LayoutArticle._field_bits["title"]

    def test_dirty_after_setattr(self):
        a = LayoutArticle()
        a.title = "New"
        assert a._dirty & LayoutArticle._field_bits["title"]
        assert not (a._dirty & LayoutArticle._field_bits["views"])

    def test_multiple_dirty_fields(self):
        a = LayoutArticle()
        a.title = "A"
        a.views = 5
        expected = LayoutArticle._field_bits["title"] | LayoutArticle._field_bits["views"]
        assert a._dirty == expected

    def test_dirty_cleared_manually(self):
        a = LayoutArticle(title="x")
        a._dirty = 0
        assert a._dirty == 0

    def test_get_dirty_field_names(self):
        a = LayoutArticle(title="x", views=3)
        dirty_names = {n for n, b in LayoutArticle._field_bits.items() if a._dirty & b}
        assert "title" in dirty_names
        assert "views" in dirty_names

    def test_slot_writes_do_not_set_dirty(self):
        """Writing to _values/_dirty/_adding/_db must not recurse or set dirty bits."""
        a = LayoutArticle()
        a._dirty = 0b1111
        a._adding = False
        a._db = "replica"
        # These are slot writes — should not affect _dirty
        assert a._dirty == 0b1111
        assert a._adding == False
        assert a._db == "replica"


class TestGetattr:
    def test_field_value_readable_via_attribute(self):
        a = LayoutArticle()
        a._values["title"] = "direct"
        assert a.title == "direct"

    def test_missing_field_raises_attribute_error(self):
        a = LayoutArticle()
        with pytest.raises(AttributeError):
            _ = a.nonexistent_field

    def test_pk_property(self):
        a = LayoutArticle()
        a._values["id"] = 42
        assert a.pk == 42


class TestRepr:
    def test_repr_shows_pk(self):
        a = LayoutArticle()
        a._values["id"] = 7
        assert repr(a) == "<LayoutArticle: pk=7>"

    def test_repr_unknown_pk(self):
        a = LayoutArticle()
        assert repr(a) == "<LayoutArticle: pk=?>"
