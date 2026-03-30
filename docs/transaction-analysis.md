# Transaction System Analysis: Django → ormpi

## Overview

This document analyzes how Django implements transactions and outlines what ormpi
needs to adopt. The key requirement is:

> **Transactions should be opt-in at the parent level. All child operations
> (save, delete, update, select_for_update) automatically execute under the
> same session without any changes to their call sites.**

---

## 1. Django's Transaction Architecture

### 1.1 Thread-Local Connection

Django's core insight is deceptively simple: every thread gets exactly one
connection object (via `threading.local`), and that single object carries all
transaction state. Because a request is handled by one thread start-to-finish,
every DB call in that request naturally shares the same connection and therefore
the same transaction.

```
Thread A: connection_A  (autocommit=False, in_atomic_block=True, savepoint_ids=[sid1])
Thread B: connection_B  (autocommit=True,  in_atomic_block=False, savepoint_ids=[])
```

There are no locks, no passing of handles — state lives on the connection.

### 1.2 `atomic()` as a State Machine

`atomic()` works as both a context manager and a decorator. It drives a state
machine on the connection:

```
Autocommit mode (initial state)
        │
        ▼  with atomic():
in_atomic_block=True, autocommit=False
BEGIN issued
        │
        ├── with atomic():  (nested)
        │     SAVEPOINT s<tid>_x1 issued
        │     savepoint_ids = [sid1]
        │          │
        │          ├── success → RELEASE SAVEPOINT sid1
        │          └── error   → ROLLBACK TO SAVEPOINT sid1 → re-raise
        │
        ├── success → COMMIT
        └── error   → ROLLBACK
                        │
                        ▼
              autocommit=True restored
```

Key connection flags:

| Flag | Meaning |
|------|---------|
| `in_atomic_block` | Currently inside at least one `atomic()` |
| `savepoint_ids` | Stack of savepoint IDs (one per nesting level) |
| `needs_rollback` | Transaction is broken; must roll back before commit |
| `commit_on_exit` | Outer block should commit on clean exit |
| `atomic_blocks` | Stack of `Atomic` instances (for durable-block validation) |

### 1.3 Savepoint Nesting

```python
with atomic():            # → BEGIN
    obj1.save()
    with atomic():        # → SAVEPOINT s123_x1
        obj2.save()
    # → RELEASE SAVEPOINT s123_x1
    with atomic():        # → SAVEPOINT s123_x2
        obj3.save()
        raise ValueError  # → ROLLBACK TO SAVEPOINT s123_x2 → re-raise
# → ROLLBACK (outer catches the re-raised exception)
```

- Nested `atomic()` **never** opens a new transaction — it creates a savepoint.
- The `savepoint_ids` list is a stack; each nesting level pushes one entry
  (`None` if savepoints are disabled).
- On rollback the savepoint is released after rolling back (no SQL dangling).

### 1.4 Thread Safety

- **No locks needed for transaction state** — it's thread-local.
- Locks exist only around the `allow_thread_sharing` counter, which is an
  opt-in escape hatch for test utilities.
- `validate_thread_sharing()` prevents using a connection from the wrong thread:
  ```python
  if not (self.allow_thread_sharing or self._thread_ident == current_thread_ident):
      raise DatabaseError("Object was created in thread X, used in thread Y")
  ```

### 1.5 On-Commit Hooks

```python
on_commit(func, using='default')
```

- Registered as `(sids: set, func, robust)` tuples on the connection.
- On savepoint rollback: hooks whose `sids` contains the rolled-back savepoint
  ID are removed.
- On final `COMMIT`: all remaining hooks run in registration order.
- `robust=True`: errors are logged but don't propagate.

### 1.6 Autocommit vs Atomic

```
autocommit=True   → each statement is its own transaction (default Django setting)
with atomic():    → sets autocommit=False, issues BEGIN
exit atomic():    → issues COMMIT or ROLLBACK, restores autocommit=True
```

Model.save() and QuerySet.update() do **not** start their own transactions —
they rely on the caller to wrap them in `atomic()` if atomicity is needed.

---

## 2. Where ormpi Currently Stands

### Current `OrmpiTransaction` (ormpi-core/src/transaction.rs)

```rust
pub struct OrmpiTransaction {
    pub handle: TransactionHandle,   // sqlx::Transaction<'static, DB>
    pub alias: String,
    pub state: TransactionState,
}
```

- `begin()` acquires a transaction from the pool and unsafely transmutes the
  lifetime to `'static`.
- `commit()` and `rollback()` consume `self`.
- `execute_raw()` allows running SQL (e.g. savepoint statements) on the handle.

### Current `save_instance` / `delete_instance` (model/executor.rs)

```rust
pub async fn save_instance(pool: &OrmpiPool, ...) -> Result<OrmpiValue>
```

Every operation takes `&OrmpiPool` and executes against the pool directly.
There is **no path** to pass a transaction handle in.

### Gap Summary

| Capability | Django | ormpi now |
|------------|--------|-----------|
| Single session shared across nested calls | ✅ thread-local | ❌ always uses pool |
| Savepoint nesting | ✅ automatic | ❌ manual SQL only |
| Operations unaware of transaction context | ✅ (reads thread-local) | ❌ (always pool) |
| Automatic rollback on error | ✅ `__exit__` | ❌ caller must call rollback |

---

## 3. The Core Problem in Async Rust

Django's trick — "every call on the same thread uses the same connection" — does
not transfer to async Rust because:

1. A `Future` can be polled on any thread.
2. `sqlx::Pool` hands out connections dynamically; you cannot guarantee the same
   connection across multiple `.await` points.
3. `sqlx::Transaction<'_, DB>` **borrows** a connection; you cannot hold it and
   simultaneously pass `&Pool` to a nested call.

The two canonical solutions in sqlx-based code:

### Solution A — Explicit `Executor` parameter (recommended)

Pass an `Executor` (either pool or transaction) down the call stack.

```rust
// sqlx's Executor trait works for both Pool and &mut Transaction
pub async fn save_instance<'e, E>(executor: E, ...) -> Result<OrmpiValue>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
```

**Pros:** Explicit, no global state, easy to reason about.
**Cons:** Every function signature changes; multi-backend dispatch becomes harder.

### Solution B — Task-local ambient transaction

Use `tokio::task_local!` to store the current transaction handle for the running
task, analogous to Django's `threading.local`.

```rust
tokio::task_local! {
    static CURRENT_TXN: RefCell<Option<TransactionHandle>>;
}
```

**Pros:** Call sites are unchanged (same as Django's UX).
**Cons:** Requires `unsafe` or `RefCell` inside task-local; harder to propagate
across spawned sub-tasks.

---

## 4. Recommended Design for ormpi

### 4.1 `ExecutorRef` — A Backend-Erased Executor

Instead of changing every function to be generic over `E: Executor`, introduce
an enum that can hold either a pool reference or a mutable transaction reference:

```rust
pub enum ExecutorRef<'a> {
    Pool(&'a OrmpiPool),
    Transaction(&'a mut OrmpiTransaction),
}
```

All DB-touching functions accept `ExecutorRef` instead of `&OrmpiPool`:

```rust
pub async fn save_instance(exec: ExecutorRef<'_>, ...) -> Result<OrmpiValue>
pub async fn delete_instance(exec: ExecutorRef<'_>, ...) -> Result<(usize, ...)>
```

The Python layer constructs an `ExecutorRef::Pool` by default. When the user
wraps code in `with transaction.atomic():`, the Python layer acquires an
`OrmpiTransaction` and passes `ExecutorRef::Transaction` instead.

### 4.2 Transaction Block (Python-side)

```python
# Python API (conceptual)
async with ormpi.atomic() as txn:
    await obj1.asave(using=txn)        # passes ExecutorRef::Transaction
    await obj2.asave(using=txn)        # same transaction
    # commit on clean exit, rollback on exception
```

Or — closer to Django's zero-callsite-change UX:

```python
async with ormpi.atomic():
    await obj1.asave()   # picks up ambient txn from task-local
    await obj2.asave()   # same
```

### 4.3 Savepoint Nesting

```rust
impl OrmpiTransaction {
    pub async fn savepoint(&mut self) -> Result<String> {
        let sid = format!("sp_{}", self.savepoint_counter);
        self.savepoint_counter += 1;
        self.execute_raw(&format!("SAVEPOINT {sid}")).await?;
        self.savepoint_ids.push(sid.clone());
        Ok(sid)
    }

    pub async fn release_savepoint(&mut self, sid: &str) -> Result<()> {
        self.execute_raw(&format!("RELEASE SAVEPOINT {sid}")).await?;
        self.savepoint_ids.retain(|s| s != sid);
        Ok(())
    }

    pub async fn rollback_to_savepoint(&mut self, sid: &str) -> Result<()> {
        self.execute_raw(&format!("ROLLBACK TO SAVEPOINT {sid}")).await?;
        // Also release it (matches Django behaviour)
        self.execute_raw(&format!("RELEASE SAVEPOINT {sid}")).await?;
        self.savepoint_ids.retain(|s| s != sid);
        Ok(())
    }
}
```

Nesting is managed by an `AtomicBlock` guard (analogous to Django's `Atomic`):

```rust
pub struct AtomicBlock<'txn> {
    txn: &'txn mut OrmpiTransaction,
    savepoint_id: Option<String>,  // None when outermost
    committed: bool,
}

impl<'txn> AtomicBlock<'txn> {
    pub async fn commit(mut self) -> Result<()> { ... }
    // Drop impl issues rollback if !committed
}
```

### 4.4 State Required on `OrmpiTransaction`

```rust
pub struct OrmpiTransaction {
    pub handle: TransactionHandle,
    pub alias: String,
    pub state: TransactionState,
    // ── new fields ──
    savepoint_counter: u32,
    savepoint_ids: Vec<String>,        // stack
    needs_rollback: bool,              // poison flag
    on_commit_hooks: Vec<Box<dyn FnOnce() + Send>>,
}
```

### 4.5 Thread Safety in Async Context

- `OrmpiTransaction` should be `!Sync` (cannot be shared across tasks
  simultaneously) — `sqlx::Transaction` already enforces this.
- Pass by `&mut` only; no `Arc<Mutex<>>` needed for the happy path.
- If cross-task sharing is required, wrap in `Arc<Mutex<OrmpiTransaction>>` at
  the call site — this is an explicit, visible decision.

---

## 5. What NOT to Copy from Django

| Django pattern | Why it doesn't apply |
|----------------|----------------------|
| `threading.local` for connection | Rust/async uses task-locals or explicit passing |
| `unsafe transmute` lifetime extension | Already present in ormpi; acceptable but should be removed when sqlx stabilises `'static` transactions |
| `autocommit` toggling | sqlx manages connection autocommit internally; we don't need to toggle |
| `closed_in_transaction` flag | sqlx's connection pool handles reconnection; not our concern |
| `errors_occurred` flag | sqlx raises errors immediately; no silent "broken" state |

---

## 6. Implementation Checklist

- [ ] Add `ExecutorRef<'a>` enum in `ormpi-core/src/transaction.rs`
- [ ] Update `save_instance`, `delete_instance`, `fetch_instance`,
      `execute_sql`, `fetch_all_raw` to accept `ExecutorRef`
- [ ] Add savepoint methods to `OrmpiTransaction`
  - `savepoint()` → creates and pushes a savepoint, returns ID
  - `release_savepoint(sid)` → releases on success
  - `rollback_to_savepoint(sid)` → rolls back on error
- [ ] Add `needs_rollback: bool` flag to `OrmpiTransaction`
- [ ] Add `on_commit_hooks` support (optional, post-MVP)
- [ ] Expose `atomic()` / `atomic_block()` context manager in Python bindings
  - Outermost call: `OrmpiTransaction::begin()`
  - Nested call: `OrmpiTransaction::savepoint()`
- [ ] Remove `unsafe transmute` in `OrmpiTransaction::begin()` — use
      `sqlx::Pool::begin()` and hold the transaction as an owned value with
      the pool kept alive separately (no lifetime extension needed when
      transaction owns its connection handle)

---

## 7. Behaviour Parity Table

| Scenario | Django | ormpi target |
|----------|--------|-------------|
| No transaction | autocommit per statement | pool per statement |
| `with atomic():` | BEGIN + COMMIT/ROLLBACK | txn.begin() + commit/rollback |
| Nested `with atomic():` | SAVEPOINT + RELEASE/ROLLBACK TO | savepoint() + release/rollback_to |
| Exception in nested block | ROLLBACK TO SAVEPOINT, re-raise | rollback_to_savepoint, propagate |
| Exception in outer block | ROLLBACK | txn.rollback() (drop guard) |
| `on_commit(fn)` | deferred until COMMIT | post-MVP |
| Thread/task isolation | `threading.local` | explicit `&mut` / task-local |
| Manual `set_rollback(True)` | marks txn as broken | `txn.needs_rollback = true` |

---

## 8. Key Takeaways

1. **One session per logical unit of work.** Django enforces this via
   thread-local; ormpi should enforce it via `ExecutorRef` passed through
   the call chain (or task-local as an ergonomic layer on top).

2. **Savepoints are the nesting primitive.** Never open a second `BEGIN` inside
   an existing transaction — always use `SAVEPOINT`.

3. **The "needs_rollback" poison flag** prevents silent data corruption: once
   something goes wrong inside a nested block, the outer block must rollback
   even if the outer code catches the exception.

4. **Operations should be transaction-agnostic.** `save_instance` should not
   know or care whether it's running inside a transaction. The caller decides
   by passing the right `ExecutorRef`.

5. **No implicit global state.** Django's thread-local is effectively global
   state. In async Rust, explicit `&mut txn` is safer and more composable.
   Task-local can be layered on top for a nicer Python API, but the Rust core
   should remain explicit.
