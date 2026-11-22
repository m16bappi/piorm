# Modern Transaction Patterns for Async-First ORMs

> Research covering SQLAlchemy 2.0, SeaORM, Tortoise ORM, Prisma, Piccolo, MikroORM,
> and the underlying sqlx Executor/Acquire primitives — with a focus on what piorm
> should adopt.

---

## 1. The Fundamental Problem

In synchronous ORMs (Django, SQLAlchemy 1.x), thread-local storage solved
transaction propagation for free: one thread = one connection = one transaction.
Every call on that thread automatically participates.

Async breaks this:

- A coroutine can be resumed on any OS thread.
- Multiple coroutines interleave on a single thread — there is no 1:1 mapping.
- `sqlx::Transaction` **borrows** a connection; you cannot hold it and
  simultaneously pass `&Pool` to a nested call.
- `asyncio.create_task()` copies the parent's `contextvars` context, but that
  copy is independent — mutations do not flow back.

Every modern async ORM has had to solve this explicitly.

---

## 2. sqlx Foundation — Executor and Acquire

Before looking at ORMs, understand the two sqlx traits everything builds on.

### `Executor<'c>` — single-query abstraction

```rust
// Works with: &Pool, &mut Connection, &mut Transaction<'_, DB>
async fn count_users<'c>(exec: impl Executor<'c, Database = Postgres>) -> i64 {
    sqlx::query_scalar!("SELECT COUNT(*) FROM users")
        .fetch_one(exec).await.unwrap()
}
```

A function taking `impl Executor` is **location-agnostic**: the caller decides
whether it runs against a pool connection or inside an existing transaction.
No signature change needed at the call site.

Limitation: `Executor` is consumed after one query. You cannot run two queries
through the same `impl Executor` parameter.

### `Acquire<'a>` — multi-query abstraction

```rust
async fn create_with_profile<'a, A>(conn: A, name: &str) -> Result<User>
where
    A: Acquire<'a, Database = Postgres>,
{
    let mut c = conn.acquire().await?;   // reusable handle

    let user = sqlx::query_as!(User, "INSERT INTO users(name) VALUES($1) RETURNING *", name)
        .fetch_one(&mut *c).await?;

    sqlx::query!("INSERT INTO profiles(user_id) VALUES($1)", user.id)
        .execute(&mut *c).await?;

    Ok(user)
}

// Same function, both work:
create_with_profile(&pool, "alice").await?;
create_with_profile(&mut *txn, "bob").await?;
```

`Acquire` is the gold standard. `&Pool` acquires a connection from the pool.
`&mut Transaction` reuses the transaction's connection. The function cannot tell
the difference. **SeaORM's `ConnectionTrait` is a higher-level ORM wrapper of
this exact pattern.**

---

## 3. SeaORM — ConnectionTrait (Rust, Recommended Reference)

### The core abstraction

```rust
// Both DatabaseConnection and DatabaseTransaction implement ConnectionTrait
let user = User::find_by_id(1).one(&db).await?;    // &DatabaseConnection
let user = User::find_by_id(1).one(&txn).await?;   // &DatabaseTransaction
```

Operations take `impl ConnectionTrait`. The caller decides context. **No
signature change when moving code from non-transactional to transactional.**

### Two transaction APIs

**Closure-based (preferred — avoids lifetime noise):**

```rust
db.transaction::<_, (), DbErr>(|txn| {
    Box::pin(async move {
        bakery.save(txn).await?;
        cake.save(txn).await?;
        Ok(())
    })
})
.await?;
// Ok → COMMIT, Err → ROLLBACK (automatic)
```

**Explicit control (when closures conflict with borrows):**

```rust
let txn = db.begin().await?;
bakery.save(&txn).await?;
txn.commit().await?;
// Drop without commit → async ROLLBACK task spawned automatically
```

### Savepoints (nested transactions)

```rust
db.transaction::<_, (), DbErr>(|txn| {
    Box::pin(async move {
        create_user(txn).await?;

        let _ = txn.transaction::<_, (), DbErr>(|nested| {
            Box::pin(async move {
                risky_op(nested).await   // Err → ROLLBACK TO SAVEPOINT
            })
        }).await;
        // outer txn still intact

        Ok(())
    })
})
.await?;
```

Nested `.transaction()` on an existing `DatabaseTransaction` automatically
issues `SAVEPOINT` rather than `BEGIN`.

### SeaORM 2.0 — generic over ConnectionTrait

```rust
async fn perform<C: ConnectionTrait + TransactionTrait>(
    db: &C,
) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    // works identically for DatabaseConnection and DatabaseTransaction
    txn.commit().await
}
```

**Key takeaway:** The `ConnectionTrait` abstraction is the cleanest Rust
approach. Operations are written once and work in all contexts.

---

## 4. SQLAlchemy 2.0 — AsyncSession (Python Reference)

### Session-per-task, autobegin

Each `AsyncSession` owns one connection. The first DB operation automatically
begins a transaction ("autobegin"). Everything after is in that transaction until
`commit()` or `rollback()`.

```python
async with async_sessionmaker(engine)() as session:
    async with session.begin():          # explicit transaction boundary
        session.add(User(name="alice"))
        session.add(Post(title="hello"))
    # COMMIT on clean exit, ROLLBACK on exception
```

The two-level nesting — `async with session` (lifecycle) + `async with
session.begin()` (transaction) — is canonical.

### Explicit passing (SQLAlchemy's recommendation)

```python
# Dependency injection via FastAPI
async def get_session() -> AsyncGenerator[AsyncSession, None]:
    async with async_sessionmaker(engine)() as session:
        async with session.begin():
            yield session

# Service layer accepts session explicitly
async def create_user(session: AsyncSession, name: str) -> User:
    user = User(name=name)
    session.add(user)
    await session.flush()   # pushes to DB, not yet committed
    return user

# Route — session flows from DI
@router.post("/users")
async def route(name: str, session: AsyncSession = Depends(get_session)):
    return await create_user(session, name)
```

SQLAlchemy's own docs explicitly **warn against** `async_scoped_session`
(their ambient ContextVar approach) for new code: it relies on mutable global
state that must be explicitly torn down. **Explicit passing is recommended.**

### Savepoints via `begin_nested()`

```python
async with session.begin():
    session.add(User(name="outer"))

    try:
        async with session.begin_nested():   # SAVEPOINT sp1
            session.add(User(name="inner"))
            raise ValueError()
    except ValueError:
        pass   # ROLLBACK TO SAVEPOINT sp1 — "inner" user gone

    # "outer" user still pending, will be committed
```

Objects modified since the savepoint are **expired** on rollback (must
re-fetch for attribute access).

---

## 5. Tortoise ORM — ContextVar Ambient Propagation

### The pattern: zero signature change

Tortoise stores the active connection in `contextvars.ContextVar`. Every model
query checks this variable before choosing between pool and transaction:

```python
# Internal (simplified)
_connection: ContextVar[Optional[BaseDBAsyncClient]] = ContextVar('connection', default=None)

async def _get_connection(model):
    conn = _connection.get()   # active transaction conn, or None
    return conn if conn else connections.get(model._meta.db)
```

Usage:

```python
async def create_user(name: str):   # NO transaction param
    await User.create(name=name)

async def create_post(title: str):  # NO transaction param
    await Post.create(title=title)

# Both participate in the same transaction automatically:
async with in_transaction():
    await create_user("alice")    # uses ambient txn
    await create_post("hello")    # same txn, same conn
```

### `@atomic()` decorator

```python
@atomic()
async def transfer(from_id: int, to_id: int, amount: Decimal):
    src = await Account.get(id=from_id).select_for_update()
    dst = await Account.get(id=to_id)
    src.balance -= amount
    dst.balance += amount
    await src.save()   # ambient txn
    await dst.save()   # same ambient txn
    # exception → ROLLBACK, clean exit → COMMIT
```

### Automatic savepoint nesting

`in_transaction()` inside an active transaction issues `SAVEPOINT` not `BEGIN`:

```python
async with in_transaction():           # BEGIN
    await create_user("outer")

    try:
        async with in_transaction():   # SAVEPOINT sp1 (detected nesting)
            await create_user("inner")
            raise ValueError()
    except ValueError:
        pass                           # ROLLBACK TO SAVEPOINT sp1

# COMMIT — only "outer" user saved
```

### Task isolation

Python 3.7+ `ContextVar` gives each `asyncio.create_task()` a **copy** of the
current context. A child task starting inside a transaction does NOT inherit
the parent's transaction connection. This is a safety guarantee, not a bug.

---

## 6. Prisma — Scoped TransactionClient (TypeScript)

### Interactive transaction (callback-scoped client)

```typescript
await prisma.$transaction(async (tx) => {
    const user = await tx.user.create({ data: { name: "bob" } });
    // use result immediately:
    await tx.post.create({ data: { title: "hi", authorId: user.id } });
}, {
    maxWait: 5000,
    timeout: 10000,
    isolationLevel: Prisma.TransactionIsolationLevel.Serializable,
});
// COMMIT on clean return, ROLLBACK on throw
```

`tx` is a scoped client clone bound to the transaction's connection. The
connection is held open for the entire callback duration.

### Propagation: explicit passing

```typescript
async function createUserWithProfile(
    tx: Prisma.TransactionClient,   // accepts both PrismaClient and TransactionClient
    name: string,
) {
    const user = await tx.user.create({ data: { name } });
    await tx.profile.create({ data: { userId: user.id } });
    return user;
}

await prisma.$transaction(async (tx) => {
    await createUserWithProfile(tx, "alice");
});
```

The type `PrismaClient | Prisma.TransactionClient` enables functions that work
both transactionally and non-transactionally.

**No savepoints.** Nested `$transaction()` inside another throws. This is a known
limitation (tracked in issue #19346).

---

## 7. MikroORM — Propagation Modes (TypeScript)

MikroORM has the most sophisticated propagation model in any TypeScript ORM,
inspired by Spring's `@Transactional`.

### EntityManager fork per transaction

```typescript
await em.transactional(async (innerEm) => {
    // innerEm is a fork with its own identity map / Unit of Work
    const user = await innerEm.findOne(User, 1);
    user.name = "updated";
    // auto-flushes before commit
});
```

### Seven propagation modes

| Mode | Behavior |
|------|---------|
| `NESTED` (default for `.transactional()`) | SAVEPOINT if in txn, else BEGIN |
| `REQUIRED` (default for `@Transactional()`) | JOIN existing txn, else BEGIN |
| `REQUIRES_NEW` | Always a new, independent transaction |
| `SUPPORTS` | Use existing txn if present; else non-transactional |
| `MANDATORY` | Must have existing txn or throw |
| `NOT_SUPPORTED` | Suspend current txn; run non-transactionally |
| `NEVER` | Throw if currently in a txn |

```typescript
@Transactional({ propagation: Propagation.REQUIRES_NEW })
async createAuditLog(entry: AuditEntry): Promise<void> {
    // Always its own transaction — never rolls back with the parent
    await this.em.persistAndFlush(entry);
}
```

`REQUIRES_NEW` is critical for audit logging, notifications, or any operation
that must persist even when the parent transaction fails.

---

## 8. Piccolo — Cleanest ContextVar Implementation (Python)

Piccolo's blog explicitly documents three approaches and ranks them.

### The three patterns

**Pattern 1 — collect-and-fire (limited):**
Batch queries then execute all at once. Cannot use intermediate results.

**Pattern 2 — explicit passing (verbose but clear):**
```python
# Every call site must pass the transaction
await User.insert(User(name="alice")).run(in_pool=False, connection=conn)
```

**Pattern 3 — ContextVar ambient (recommended):**
```python
from contextvars import ContextVar
from piccolo.engine import engine

_current_conn: ContextVar = ContextVar('piccolo_conn', default=None)

class Transaction:
    async def __aenter__(self):
        self.conn = await engine.get_connection()
        await self.conn.execute("BEGIN")
        self.token = _current_conn.set(self.conn)
        return self

    async def __aexit__(self, exc_type, exc, tb):
        conn = _current_conn.get()
        if exc_type:
            await conn.execute("ROLLBACK")
        else:
            await conn.execute("COMMIT")
        await conn.close()
        _current_conn.reset(self.token)  # restore prior state cleanly
```

Any query uses the ambient connection transparently:
```python
async def fetch_employers():
    return await Employer.select().run()   # no txn param

async def fetch_people():
    return await Person.select().run()     # no txn param

async with Transaction():
    employers = await fetch_employers()    # ambient txn
    people = await fetch_people()          # same txn
```

The `reset(self.token)` call is critical — it restores the outer context's
value, enabling correct nesting of transactions without global state.

---

## 9. Pattern Comparison

### Explicit passing vs ambient ContextVar

| Dimension | Explicit Passing | Ambient ContextVar |
|-----------|-----------------|-------------------|
| Signature change on callee | Yes — every fn takes session/txn | No — callee unchanged |
| Task safety | Trivially safe | Safe with `ContextVar` (not ThreadLocal) |
| Testability | Easy — inject mock | Requires test to set up context |
| Refactoring cost | High — adding txn propagates upward | Low — wrap top-level only |
| Debug traceability | Complete — data flow visible | Implicit — harder to trace |
| Nested partial rollback | Caller decides savepoint scope | Inner context manager detects nesting |
| Recommended by | SQLAlchemy, Prisma, SeaORM, sqlx | Tortoise ORM, Piccolo, Django |

**SQLAlchemy's explicit position** (from their 2.0 docs):
> "pass the `AsyncSession` directly to the awaitable functions that need it"
> — the `async_scoped_session` approach is explicitly discouraged.

**When ambient wins:** Python layer ergonomics. If the ORM is consumed by
application code that already has service layers (like Django's view/service
pattern), ambient ContextVar eliminates all parameter threading and gives the
exact same DX as Django's `@atomic`. The Rust core can remain explicit while
the Python API layer adds ambient propagation on top.

### Connection lifetime models

| Model | Examples | Trade-off |
|-------|----------|-----------|
| Connection-per-session | SQLAlchemy Session | Connection held for session lifetime; ideal for short sessions |
| Connection-per-request | Django | Connection held for request; returns to pool between requests |
| Connection-per-transaction | Prisma | Acquired on transaction enter, released on exit; minimal hold time |

Prisma's connection-per-transaction model is most pool-friendly but prevents
reading outside a transaction without acquiring a separate connection.

---

## 10. Savepoints — Non-Negotiable

Every production ORM (SQLAlchemy, SeaORM, Tortoise, MikroORM, Django) implements
nested transactions as `SAVEPOINT` / `ROLLBACK TO SAVEPOINT` / `RELEASE SAVEPOINT`.

Why this matters:

```
BEGIN
  INSERT user                ← must persist
  SAVEPOINT sp1
    INSERT payment           ← may fail (card declined)
  ROLLBACK TO SAVEPOINT sp1  ← payment rolled back
  RELEASE SAVEPOINT sp1
  INSERT audit_log           ← still in outer transaction
COMMIT
```

Without savepoints, a failure in any nested operation forces a full transaction
rollback — making partial success impossible.

The nesting rule: if you enter a transaction context and a transaction is already
active, issue `SAVEPOINT`, not `BEGIN`.

---

## 11. Drop-Based Rollback — Correctness Guarantee

sqlx and SeaORM implement rollback on `Drop` for `DatabaseTransaction`:

```rust
impl Drop for DatabaseTransaction {
    fn drop(&mut self) {
        if !self.committed {
            // Spawn a local task to execute ROLLBACK
            // Connection is marked RollingBack — subsequent use awaits rollback
        }
    }
}
```

This means: **forgetting to call `rollback()` is not a bug** — the transaction
rolls back automatically when it goes out of scope. This is the Rust equivalent
of Django's `__exit__` rolling back on exception.

Python equivalent: `__aexit__` on the transaction context manager calls
`ROLLBACK` if `commit()` was never called.

---

## 12. Synthesis — What piorm Should Adopt

### Rust core layer

1. **`ConnectionTrait` / `ExecutorRef` abstraction** (SeaORM model)
   - `PiormPool` and `PiormTransaction` both implement a common `Connectable` trait
   - All operation functions (`save_instance`, `delete_instance`, etc.) accept
     `&dyn Connectable` — no signature change needed when adding transactions

2. **Closure-based `transaction()` as primary API** (SeaORM model)
   ```rust
   pool.transaction(|txn| async move {
       model_a.save(txn).await?;
       model_b.save(txn).await?;
       Ok(())
   }).await?;
   ```
   Auto-commit on `Ok`, auto-rollback on `Err`.

3. **Explicit `begin()`/`commit()`/`rollback()` as secondary API** for cases
   where closures fight with lifetimes.

4. **Savepoints as automatic nesting primitive**
   - `transaction()` called on a `DatabaseTransaction` issues `SAVEPOINT`
   - Never issues nested `BEGIN`

5. **Drop-based rollback** — uncommitted `PiormTransaction` rolls back
   automatically on drop (spawns Tokio local task for async `ROLLBACK`).

6. **Remove `unsafe transmute`** — hold the pool alive separately and let sqlx
   own the transaction lifetime, no `'static` extension needed.

### Python layer

1. **`async with atomic() as txn`** — explicit style, `txn` passed to operations
   ```python
   async with piorm.atomic() as txn:
       await obj1.asave(using=txn)
       await obj2.asave(using=txn)
   ```

2. **`@atomic()` decorator + `ContextVar` ambient** — zero-signature-change style
   ```python
   @piorm.atomic()
   async def transfer(from_id, to_id, amount):
       src = await Account.objects.select_for_update().get(id=from_id)
       dst = await Account.objects.get(id=to_id)
       src.balance -= amount
       dst.balance += amount
       await src.asave()   # picks up ambient txn
       await dst.asave()   # same txn
   ```

3. **Automatic savepoint nesting** — entering `atomic()` inside an active
   transaction issues `SAVEPOINT` not `BEGIN`

4. **`REQUIRES_NEW` escape hatch** — for audit logs and notifications that must
   commit independently:
   ```python
   @piorm.atomic(propagation="requires_new")
   async def log_event(event):
       await AuditLog.create(**event)
   ```

### What to skip

| Pattern | Reason to skip |
|---------|----------------|
| `scoped_session` / global ambient | Requires explicit teardown; SQLAlchemy deprecated it |
| Thread-local connection | Not applicable in async; each task needs its own |
| Autocommit toggling | sqlx manages this internally |
| `closed_in_transaction` flag | sqlx pool handles reconnect |
| Silent error swallowing | Propagate all errors; use `robust=True` only for on-commit hooks |

---

## 13. Reference Architecture Diagram

```
Python API Layer
┌──────────────────────────────────────────────────────────┐
│  @atomic()  / async with atomic() as txn                 │
│  ContextVar stores active PiormTransaction               │
│  auto savepoint on nested atomic()                       │
└──────────────────┬───────────────────────────────────────┘
                   │ calls into PyO3 bindings
Rust Core Layer
┌──────────────────▼───────────────────────────────────────┐
│  save_instance(exec: &dyn Connectable, ...)              │
│  delete_instance(exec: &dyn Connectable, ...)            │
│  fetch_instance(exec: &dyn Connectable, ...)             │
│                                                          │
│  Connectable                                             │
│  ├── PiormPool        → acquires from sqlx Pool          │
│  └── PiormTransaction → uses borrowed sqlx Transaction   │
│                                                          │
│  PiormTransaction                                        │
│  ├── begin()      → BEGIN                                │
│  ├── savepoint()  → SAVEPOINT sp_N                       │
│  ├── commit()     → COMMIT                               │
│  ├── rollback()   → ROLLBACK                             │
│  └── Drop impl    → async ROLLBACK if not committed      │
└──────────────────────────────────────────────────────────┘
```

---

## Sources

- [SQLAlchemy 2.0 — Transactions and Connection Management](https://docs.sqlalchemy.org/en/20/orm/session_transaction.html)
- [SQLAlchemy 2.0 — Asynchronous I/O](https://docs.sqlalchemy.org/en/20/orm/extensions/asyncio.html)
- [SeaORM — Transaction](https://www.sea-ql.org/SeaORM/docs/advanced-query/transaction/)
- [SeaORM 2.0 release blog](https://www.sea-ql.org/blog/2025-09-24-sea-orm-2.0/)
- [Tortoise ORM — Transactions](https://tortoise.github.io/transactions.html)
- [Prisma — Transactions and batch queries](https://www.prisma.io/docs/orm/prisma-client/queries/transactions)
- [MikroORM — Transactions and Concurrency](https://mikro-orm.io/docs/transactions)
- [Piccolo ORM — Transactions](https://piccolo-orm.com/blog/piccolo-transactions/)
- [sqlx — Acquire trait](https://docs.rs/sqlx/latest/sqlx/trait.Acquire.html)
- [sqlx — Executor trait](https://docs.rs/sqlx/latest/sqlx/trait.Executor.html)
