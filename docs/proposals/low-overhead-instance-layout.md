# Proposal: Low-Overhead Instance Layout — `__slots__` + Bitmask Dirty Tracking

## Problem

Every piorm model instance currently allocates **4 Python containers** for bookkeeping:

```
instance.__dict__        ← ~72B (always exists)
  ├── "_piorm_values"    ← dict: field values              ~72B + entries
  ├── "_piorm_dirty"     ← set:  dirty field names         ~216B base
  └── "_piorm_state"     ← dict: {"adding": bool, "db": str|None}  ~72B + entries
```

**~330B overhead per instance** before any field data is stored. At 1M instances that's ~330MB of pure infrastructure.

---

## Research: How Other ORMs Do It

| ORM | Field Storage | `__slots__` | Dirty Tracking | Est. Overhead |
|-----|--------------|-------------|----------------|---------------|
| GINO | `__values__` dict | No | None (explicit UpdateRequest) | ~150B |
| Tortoise | `__dict__` directly | No | None | ~200B |
| **Pony** | `_vals_` dict in slot | **Yes** | `_wbits_` bitmask | **~100B** |
| Peewee | `__data__` dict + `_dirty` set | No | `_dirty` set | ~280B |
| **piorm** | `_piorm_values` dict + set + dict | No | `_piorm_dirty` set | **~330B** |
| Django | `__dict__` + `_state` object | No | O(N) compare on save | ~380B |
| SQLAlchemy | `__dict__` + separate `InstanceState` | No (instance) | `committed_state` dict | ~600B |

Pony is the most efficient — the only ORM using `__slots__` + bitmask tracking.

---

## Proposed Data Structure

### Instance layout

```python
class Model:
    __slots__ = ("_values", "_dirty", "_adding", "_db", "__weakref__")
    _field_bits: ClassVar[dict[str, int]]   # {"title": 1, "views": 2, "author_id": 4, ...}
```

```
Model Instance (proposed)
├── _values     → dict            ← ONLY allocation (~72B + entries)
│    ├── "id"        → 1
│    ├── "title"     → "Hello"
│    ├── "views"     → 42
│    └── "author_id" → 3
│
├── _dirty      → int (bitmask)   ← 0 bytes extra (stored in slot)
├── _adding     → bool            ← 0 bytes extra (stored in slot)
├── _db         → str | None      ← 0 bytes extra (stored in slot)
└── __weakref__                   ← 0 bytes extra (required by __slots__)

Total overhead: ~72B (single dict)
```

### Class-level bit assignment

```python
# Assigned during _register_model_class
class Article(Model):
    _field_bits = {
        "id":         1 << 0,   # 1
        "title":      1 << 1,   # 2
        "views":      1 << 2,   # 4
        "author_id":  1 << 3,   # 8
    }
```

### Bitmask operations

```
Setting a field:
  _dirty |= _field_bits["title"]        →  _dirty = 0b0010

Setting another field:
  _dirty |= _field_bits["views"]        →  _dirty = 0b0110

Checking if any field is dirty:
  _dirty != 0                           →  True

Checking if "title" is dirty:
  _dirty & _field_bits["title"]         →  2 (truthy)

Getting all dirty field names:
  [name for name, bit in _field_bits.items() if _dirty & bit]
  → ["title", "views"]

Clearing after save:
  _dirty = 0                            →  0b0000
```

Supports up to 64 fields in a single `int`. Python `int` is arbitrary precision, so models with >64 fields just get a bigger integer — still O(1) per operation.

---

## Comparison: Before vs After

### Memory per instance

```
BEFORE (4 containers):                   AFTER (1 container):
────────────────────────                 ────────────────────────
__dict__       ~72B                      __slots__     0B  (eliminated)
_piorm_values  ~72B  (dict)              _values       ~72B (dict — only alloc)
_piorm_dirty   ~216B (set)               _dirty        0B  (int in slot)
_piorm_state   ~72B  (dict)              _adding       0B  (bool in slot)
                                          _db           0B  (ref in slot)
────────────────────────                 ────────────────────────
Total: ~330B, 4 allocs                   Total: ~72B, 1 alloc
```

### At scale

| Instances | Current | Proposed | Saved |
|-----------|---------|----------|-------|
| 10,000 | 3.3 MB | 0.72 MB | 2.6 MB (79%) |
| 100,000 | 33 MB | 7.2 MB | 26 MB (79%) |
| 1,000,000 | 330 MB | 72 MB | **258 MB (79%)** |

### CPU per operation

| Operation | Current | Proposed |
|-----------|---------|----------|
| Field write | `_piorm_dirty.add(name)` — hash + set insert | `_dirty \|= bit` — single OR |
| Field read | `_piorm_values[name]` — 2 lookups (`__dict__` → dict) | `_values[name]` — 1 lookup (slot → dict) |
| Is dirty? | `len(_piorm_dirty) > 0` | `_dirty != 0` — int compare |
| Is field dirty? | `name in _piorm_dirty` — hash lookup | `_dirty & bit` — int AND |
| Get dirty fields | `list(_piorm_dirty)` | comprehension over `_field_bits` |
| Clear on save | `_piorm_dirty.clear()` — set clear | `_dirty = 0` — int write |
| Hydrate from row | 3 container allocations | 1 container allocation |

---

## Rust Hydration Changes

### Current `row_to_instance` (3 allocations)

```rust
fn row_to_instance(py, row, model_class) -> PyResult<PyObject> {
    let instance = model_class.bind(py).call0()?;

    let values = PyDict::new_bound(py);               // ALLOC 1
    for (col, val) in &row.values {
        values.set_item(col, piorm_value_to_py(py, val))?;
    }
    instance.setattr("_piorm_values", &values)?;

    let state = PyDict::new_bound(py);                // ALLOC 2
    state.set_item("adding", false)?;
    state.set_item("db", py.None())?;
    instance.setattr("_piorm_state", &state)?;

    let dirty = PySet::empty_bound(py)?;              // ALLOC 3
    instance.setattr("_piorm_dirty", dirty)?;

    Ok(instance.into())
}
```

### Proposed `row_to_instance` (1 allocation)

```rust
fn row_to_instance(py, row, model_class) -> PyResult<PyObject> {
    let instance = model_class.bind(py).call0()?;

    let values = PyDict::new_bound(py);               // ONLY ALLOC
    for (col, val) in &row.values {
        values.set_item(col, piorm_value_to_py(py, val))?;
    }
    instance.setattr("_values", &values)?;

    instance.setattr("_dirty", 0u64)?;                // no alloc
    instance.setattr("_adding", false)?;              // no alloc
    instance.setattr("_db", py.None())?;              // no alloc

    Ok(instance.into())
}
```

### Save helpers

```rust
// OLD
pub(crate) fn extract_dirty(instance) -> HashSet<String> {
    let set = instance.getattr("_piorm_dirty")?.downcast::<PySet>()?;
    let mut hs = HashSet::new();
    for item in set.iter() { hs.insert(item.extract()?); }
    hs
}

// NEW
pub(crate) fn extract_dirty(instance, field_bits: &PyDict) -> HashSet<String> {
    let dirty_mask: u64 = instance.getattr("_dirty")?.extract()?;
    let mut dirty = HashSet::new();
    for (name, bit) in field_bits.iter() {
        let bit: u64 = bit.extract()?;
        if dirty_mask & bit != 0 {
            dirty.insert(name.extract()?);
        }
    }
    dirty
}

// Clear dirty after save
// OLD: instance.getattr("_piorm_dirty")?.call_method0("clear")?;
// NEW: instance.setattr("_dirty", 0u64)?;

// Update state after save
// OLD: state dict set_item("adding", false); state.set_item("db", &using);
// NEW: instance.setattr("_adding", false)?; instance.setattr("_db", &using)?;
```

---

## `__slots__` Implications

- Model instances **cannot** hold arbitrary attributes — only declared fields and the 5 slots. This is intentional.
- `__getattr__` falls through to `_values` for field reads, so `instance.title` still works.
- `__setattr__` intercepts writes, stores in `_values`, and sets the dirty bit.
- If a subclass needs dynamic attributes, it can opt in:
  ```python
  class MyModel(Model):
      __slots__ = ("__dict__",)  # opt-in to dynamic attributes
  ```

---

## Migration Map

All changes are to **private** `_piorm_*` attributes — no public API breakage.

| Old | New |
|-----|-----|
| `instance._piorm_values` | `instance._values` |
| `instance._piorm_dirty` (set) | `instance._dirty` (int bitmask) |
| `"title" in instance._piorm_dirty` | `bool(instance._dirty & Model._field_bits["title"])` |
| `instance._piorm_dirty.add("title")` | `instance._dirty \|= Model._field_bits["title"]` |
| `instance._piorm_dirty.clear()` | `instance._dirty = 0` |
| `instance._piorm_state["adding"]` | `instance._adding` |
| `instance._piorm_state["db"]` | `instance._db` |

---

## Summary

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Overhead per instance | ~330B | ~72B | **-79%** |
| Python allocations per instance | 4 | 1 | **-75%** |
| Dirty check | set hash lookup | integer AND | **faster** |
| Dirty clear | set.clear() | int = 0 | **faster** |
| GC pressure | 4 tracked objects | 1 tracked object | **-75%** |
| Public API breakage | — | None | — |

This brings piorm's per-instance overhead from the heaviest among lightweight ORMs (~330B) to the lightest (~72B), matching Pony's efficiency while keeping piorm's existing dirty-tracking semantics intact.
