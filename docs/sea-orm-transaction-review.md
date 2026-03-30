# SeaORM Transaction Review — How to Adopt It in ormpi

> Based on reading `sea-orm/src/database/{transaction.rs, connection.rs, executor.rs, db_connection.rs}`
> and the backend drivers directly from the source.

---

## Part 1: How SeaORM Actually Works

### 1.1 The Core Struct

```rust
// sea-orm/src/database/transaction.rs:21-26
pub struct DatabaseTransaction {
    conn: Arc<Mutex<InnerConnection>>,  // ← shared, Arc-clonable connection
    backend: DbBackend,
    open: bool,                          // ← commit/rollback sets this to false
    metric_callback: Option<Callback>,
}
```

Three things stand out immediately:

1. **`Arc<Mutex<InnerConnection>>`** — not `Box<sqlx::Transaction<'static, DB>>`.
   The connection is heap-allocated and reference-counted. This is how nesting
   works: a child transaction shares the **same Arc** as its parent.

2. **`open: bool`** — the only state flag. Set to `false` after `commit()` or
   `rollback()`. The `Drop` impl checks this before issuing rollback.

3. **No `unsafe`** — no lifetime transmutation anywhere in the transaction layer.

### 1.2 How Nesting Works — The Key Insight

When you call `.begin()` on a `DatabaseTransaction`, it does this:

```rust
// transaction.rs:612-622
async fn begin(&self) -> Result<DatabaseTransaction, DbErr> {
    DatabaseTransaction::begin(
        Arc::clone(&self.conn),  // ← CLONE the Arc, not the connection
        self.backend,
        self.metric_callback.clone(),
        None, None, None,
    ).await
}
```

The child transaction gets a **clone of the same Arc**. Both parent and child
point to the exact same `InnerConnection`. When `begin()` is called on that
shared connection, sqlx's `TransactionManager` increments an internal depth
counter and issues the right SQL:

```
depth == 0  →  BEGIN [DEFERRED|IMMEDIATE|EXCLUSIVE]   (SQLite specific)
depth > 0   →  SAVEPOINT sqlx_s1  (auto-generated name)
```

For SQLite, SeaORM explicitly reads the depth to decide the BEGIN statement:

```rust
// transaction.rs:97-108
let depth = <sqlx::Sqlite as sqlx::Database>::TransactionManager
    ::get_transaction_depth(c);

let statement = if depth == 0 {
    sqlite_transaction_mode.map(|mode| {
        Cow::from(format!("BEGIN {}", mode.sqlite_keyword()))
    })
} else {
    None  // let sqlx handle SAVEPOINT automatically
};

<sqlx::Sqlite as sqlx::Database>::TransactionManager::begin(c, statement).await
```

For Postgres and MySQL, no depth check is needed — sqlx handles both
`BEGIN`/`SAVEPOINT` automatically via its depth counter when the same connection
is reused.

### 1.3 MySQL vs Postgres Transaction Config Ordering

This is a subtle but critical difference SeaORM handles correctly:

```rust
// transaction.rs:64-88
InnerConnection::MySql(c) => {
    // MySQL: SET TRANSACTION must come BEFORE BEGIN
    set_transaction_config(c, isolation_level, access_mode).await?;
    TransactionManager::begin(c, None).await
}

InnerConnection::Postgres(c) => {
    // Postgres: BEGIN first, then SET TRANSACTION inside the transaction
    TransactionManager::begin(c, None).await?;
    set_transaction_config(c, isolation_level, access_mode).await
}
```

### 1.4 Drop-Based Rollback

```rust
// transaction.rs:322-326
impl Drop for DatabaseTransaction {
    fn drop(&mut self) {
        self.start_rollback().expect("Fail to rollback transaction");
    }
}

// transaction.rs:271-308
fn start_rollback(&mut self) -> Result<(), DbErr> {
    if self.open {
        if let Some(mut conn) = self.conn.try_lock() {  // ← try_lock, not lock
            match &mut *conn {
                InnerConnection::MySql(c) => {
                    TransactionManager::start_rollback(c);
                }
                InnerConnection::Postgres(c) => {
                    TransactionManager::start_rollback(c);
                }
                InnerConnection::Sqlite(c) => {
                    TransactionManager::start_rollback(c);
                }
                _ => ...
            }
        }
        // if try_lock fails → "Dropping a locked Transaction" error
    }
    Ok(())
}
```

`start_rollback` ≠ `rollback`. It marks the connection's internal state as
"needs rollback". The actual `ROLLBACK` SQL is sent on the **next async
operation** against that connection (typically when it returns to the pool).
This avoids needing an async executor inside a synchronous `Drop`.

`try_lock()` is used (not `lock().await`) because `Drop` is synchronous. If
the lock is held (impossible in normal usage since you can't hold the lock and
drop simultaneously), it returns an error.

### 1.5 The `DatabaseExecutor` Enum — The Unification Layer

```rust
// executor.rs:13-19
pub enum DatabaseExecutor<'c> {
    Connection(&'c DatabaseConnection),
    Transaction(&'c DatabaseTransaction),
    OwnedTransaction(DatabaseTransaction),
}
```

This enum implements both `ConnectionTrait` and `TransactionTrait`. Every
variant delegates to its inner type. This means **any function accepting
`&impl ConnectionTrait` works with both pool and transaction**, with zero
branching at the call site.

```rust
// executor.rs:44-74
impl ConnectionTrait for DatabaseExecutor<'_> {
    async fn execute_raw(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        match self {
            DatabaseExecutor::Connection(conn) => conn.execute_raw(stmt).await,
            DatabaseExecutor::Transaction(trans) => trans.execute_raw(stmt).await,
            DatabaseExecutor::OwnedTransaction(trans) => trans.execute_raw(stmt).await,
        }
    }
    // ... same pattern for query_one_raw, query_all_raw, execute_unprepared
}
```

### 1.6 The Trait Hierarchy

```
ConnectionTrait    — execute_raw, execute_unprepared, query_one_raw, query_all_raw
      ↑
TransactionTrait   — begin, begin_with_config, begin_with_options, transaction(closure)
      ↑
TransactionSession — commit(self), rollback(self)   [consuming methods]
```

`DatabaseTransaction` implements all three. `DatabaseConnection` implements
`ConnectionTrait` and `TransactionTrait` but not `TransactionSession` (pools
don't commit/rollback in the same sense).

`TransactionTrait` has an associated type:
```rust
type Transaction: ConnectionTrait + TransactionTrait + TransactionSession;
```

This means the returned transaction is itself capable of starting nested
transactions and being committed/rolled back.

### 1.7 The Closure API — `run()`

```rust
// transaction.rs:135-153
pub(crate) async fn run<F, T, E>(self, callback: F) -> Result<T, TransactionError<E>>
where
    F: for<'b> FnOnce(&'b DatabaseTransaction)
        -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'b>> + Send,
    T: Send,
    E: std::fmt::Display + std::fmt::Debug + Send,
{
    let res = callback(&self).await.map_err(TransactionError::Transaction);
    if res.is_ok() {
        self.commit().await.map_err(TransactionError::Connection)?;
    } else {
        self.rollback().await.map_err(TransactionError::Connection)?;
    }
    res
}
```

`TransactionTrait::transaction()` creates a new transaction then calls `run()`:

```rust
async fn transaction<F, T, E>(&self, callback: F) -> Result<T, TransactionError<E>> {
    let transaction = self.begin().await.map_err(TransactionError::Connection)?;
    transaction.run(callback).await
}
```

When called on an existing `DatabaseTransaction`, `self.begin()` clones the Arc
and issues a SAVEPOINT. `run()` then either releases or rolls back that
savepoint. The outer transaction is unaffected if the inner one fails — the
caller catches the error and continues.

### 1.8 `InnerConnection` Enum

```rust
pub(crate) enum InnerConnection {
    #[cfg(feature = "sqlx-mysql")]
    MySql(PoolConnection<sqlx::MySql>),        // ← sqlx PoolConnection

    #[cfg(feature = "sqlx-postgres")]
    Postgres(PoolConnection<sqlx::Postgres>),

    #[cfg(feature = "sqlx-sqlite")]
    Sqlite(PoolConnection<sqlx::Sqlite>),

    // ... mock, proxy, rusqlite variants
}
```

`PoolConnection<DB>` implements `DerefMut<Target = DB::Connection>`. This means
`&mut *pool_conn` gives `&mut PgConnection` / `&mut SqliteConnection` etc.,
which is exactly what sqlx's `Executor` trait and `TransactionManager` need.

---

## Part 2: What ormpi Has Today vs What Needs to Change

### Current ormpi `OrmpiTransaction`

```rust
// ormpi-core/src/transaction.rs (current)
pub struct OrmpiTransaction {
    pub handle: TransactionHandle,    // Box<sqlx::Transaction<'static, DB>>
    pub alias: String,
    pub state: TransactionState,
}

pub async fn begin(pool: &OrmpiPool, alias: &str) -> crate::Result<Self> {
    let handle = match pool {
        OrmpiPool::Sqlite(p) => {
            let txn = p.begin().await?;
            // ⚠ unsafe transmute to 'static — borrows pool lifetime
            let txn: sqlx::Transaction<'static, sqlx::Sqlite> =
                unsafe { std::mem::transmute(txn) };
            TransactionHandle::Sqlite(Box::new(txn))
        }
        // ...
    };
}
```

Problems:

| Issue | Impact |
|-------|--------|
| `unsafe transmute` to `'static` | Undefined behaviour if pool is dropped while txn is alive; sound only by convention |
| `Box<sqlx::Transaction<'static, DB>>` | Cannot share with nested transactions (no Arc) |
| No `ConnectionTrait` equivalent | All ops take `&OrmpiPool`; can't route through transaction |
| `state: TransactionState` enum | Redundant — `open: bool` is sufficient |
| No `Drop` rollback | Uncommitted transactions silently evaporate |

### Current executor functions

```rust
// model/executor.rs (current)
pub async fn save_instance(pool: &OrmpiPool, ...) -> Result<OrmpiValue>
pub async fn delete_instance(pool: &OrmpiPool, ...) -> Result<(usize, ...)>
pub async fn fetch_instance(pool: &OrmpiPool, ...) -> Result<HashMap<...>>
pub async fn execute_sql(pool: &OrmpiPool, ...) -> Result<u64>
pub async fn fetch_all_raw(pool: &OrmpiPool, ...) -> Result<Vec<OrmpiRow>>
```

Every function is hardwired to `&OrmpiPool`. There is no path to route through
a transaction.

---

## Part 3: Adoption Plan — What to Build

### 3.1 `InnerConnection` — Replaces `TransactionHandle`

```rust
// ormpi-core/src/transaction.rs (new)

pub(crate) enum InnerConnection {
    Sqlite(sqlx::pool::PoolConnection<sqlx::Sqlite>),
    Mysql(sqlx::pool::PoolConnection<sqlx::MySql>),
    Postgres(sqlx::pool::PoolConnection<sqlx::Postgres>),
}
```

Acquire via `pool.acquire().await?` instead of `pool.begin().await?`. SeaORM
acquires the connection first, then drives `TransactionManager::begin()` on the
raw connection directly. This keeps the connection reusable for depth tracking.

### 3.2 `OrmpiTransaction` — New Shape

```rust
pub struct OrmpiTransaction {
    pub(crate) conn: Arc<futures_util::lock::Mutex<InnerConnection>>,
    pub(crate) backend: DbBackend,
    pub(crate) open: bool,
    pub alias: String,
}
```

Key changes:
- `Arc<Mutex<InnerConnection>>` instead of `Box<sqlx::Transaction<'static, DB>>`
- `open: bool` instead of `TransactionState` enum
- No `unsafe` — `Arc::clone` for nesting
- `futures_util::lock::Mutex` (async-aware) not `std::sync::Mutex`

### 3.3 Begin — How to Eliminate `unsafe transmute`

SeaORM acquires a `PoolConnection` (not a `Transaction`) and calls
`TransactionManager::begin()` on the deref'd connection directly:

```rust
// What SeaORM does internally:
let mut pool_conn: PoolConnection<sqlx::Sqlite> = pool.acquire().await?;
// pool_conn derefs to &mut SqliteConnection
<sqlx::Sqlite as sqlx::Database>::TransactionManager::begin(
    &mut *pool_conn,
    statement,   // None, or "BEGIN DEFERRED" etc.
).await?;
```

For ormpi:

```rust
impl OrmpiTransaction {
    pub async fn begin(pool: &OrmpiPool, alias: &str) -> crate::Result<Self> {
        let (inner, backend) = match pool {
            OrmpiPool::Sqlite(p) => {
                let mut conn = p.acquire().await?;
                <sqlx::Sqlite as sqlx::Database>::TransactionManager::begin(
                    &mut *conn, None,
                ).await.map_err(OrmpiError::Database)?;
                (InnerConnection::Sqlite(conn), DbBackend::Sqlite)
            }
            OrmpiPool::Mysql(p) => {
                let mut conn = p.acquire().await?;
                <sqlx::MySql as sqlx::Database>::TransactionManager::begin(
                    &mut *conn, None,
                ).await.map_err(OrmpiError::Database)?;
                (InnerConnection::Mysql(conn), DbBackend::Mysql)
            }
            OrmpiPool::Postgres(p) => {
                let mut conn = p.acquire().await?;
                <sqlx::Postgres as sqlx::Database>::TransactionManager::begin(
                    &mut *conn, None,
                ).await.map_err(OrmpiError::Database)?;
                (InnerConnection::Postgres(conn), DbBackend::Postgres)
            }
        };

        Ok(OrmpiTransaction {
            conn: Arc::new(futures_util::lock::Mutex::new(inner)),
            backend,
            open: true,
            alias: alias.to_string(),
        })
    }
}
```

No `unsafe`. The `PoolConnection` owns the actual DB connection; it's stored
inside the `Arc<Mutex<>>`. The `Arc` keeps it alive for as long as any
transaction (parent or nested) references it.

### 3.4 Nested Transaction / Savepoint — Via Arc Clone

```rust
impl OrmpiTransaction {
    /// Start a nested transaction. Issues SAVEPOINT via sqlx's depth counter.
    pub async fn begin_nested(&self) -> crate::Result<OrmpiTransaction> {
        {
            let mut conn = self.conn.lock().await;
            match &mut *conn {
                InnerConnection::Sqlite(c) => {
                    <sqlx::Sqlite as sqlx::Database>::TransactionManager::begin(
                        &mut **c, None,  // None → sqlx detects depth > 0 → SAVEPOINT
                    ).await.map_err(OrmpiError::Database)?;
                }
                InnerConnection::Mysql(c) => {
                    <sqlx::MySql as sqlx::Database>::TransactionManager::begin(
                        &mut **c, None,
                    ).await.map_err(OrmpiError::Database)?;
                }
                InnerConnection::Postgres(c) => {
                    <sqlx::Postgres as sqlx::Database>::TransactionManager::begin(
                        &mut **c, None,
                    ).await.map_err(OrmpiError::Database)?;
                }
            }
        }

        Ok(OrmpiTransaction {
            conn: Arc::clone(&self.conn),  // ← share the same connection
            backend: self.backend,
            open: true,
            alias: self.alias.clone(),
        })
    }
}
```

sqlx tracks depth per-connection. Because parent and child share the same
`Arc<Mutex<InnerConnection>>`, the depth counter is shared. On nested
`begin()`, sqlx automatically issues `SAVEPOINT`. On nested `commit()`, it
issues `RELEASE SAVEPOINT`. On nested `rollback()`, it issues
`ROLLBACK TO SAVEPOINT`.

### 3.5 Commit and Rollback

```rust
pub async fn commit(mut self) -> crate::Result<()> {
    let mut conn = self.conn.lock().await;
    match &mut *conn {
        InnerConnection::Sqlite(c) => {
            <sqlx::Sqlite as sqlx::Database>::TransactionManager::commit(&mut **c)
                .await.map_err(OrmpiError::Database)?;
        }
        InnerConnection::Mysql(c) => {
            <sqlx::MySql as sqlx::Database>::TransactionManager::commit(&mut **c)
                .await.map_err(OrmpiError::Database)?;
        }
        InnerConnection::Postgres(c) => {
            <sqlx::Postgres as sqlx::Database>::TransactionManager::commit(&mut **c)
                .await.map_err(OrmpiError::Database)?;
        }
    }
    self.open = false;
    Ok(())
}

pub async fn rollback(mut self) -> crate::Result<()> {
    let mut conn = self.conn.lock().await;
    match &mut *conn {
        InnerConnection::Sqlite(c) => {
            <sqlx::Sqlite as sqlx::Database>::TransactionManager::rollback(&mut **c)
                .await.map_err(OrmpiError::Database)?;
        }
        // ... mysql, postgres same pattern
    }
    self.open = false;
    Ok(())
}
```

### 3.6 Drop-Based Rollback

```rust
impl Drop for OrmpiTransaction {
    fn drop(&mut self) {
        if self.open {
            // try_lock: synchronous, non-blocking
            if let Some(mut conn) = self.conn.try_lock() {
                match &mut *conn {
                    InnerConnection::Sqlite(c) => {
                        <sqlx::Sqlite as sqlx::Database>::TransactionManager
                            ::start_rollback(&mut **c);
                    }
                    InnerConnection::Mysql(c) => {
                        <sqlx::MySql as sqlx::Database>::TransactionManager
                            ::start_rollback(&mut **c);
                    }
                    InnerConnection::Postgres(c) => {
                        <sqlx::Postgres as sqlx::Database>::TransactionManager
                            ::start_rollback(&mut **c);
                    }
                }
            }
            // If try_lock fails, the connection will rollback when returned to pool
        }
    }
}
```

`start_rollback` (not `rollback`) marks the connection state. The actual
`ROLLBACK` SQL runs the next time the connection is used — when
`PoolConnection` drops and the connection returns to the pool, sqlx sees
the pending rollback and executes it.

### 3.7 `OrmpiConnectionTrait` — The Unification Trait

```rust
// ormpi-core/src/connection.rs (new file)
#[async_trait::async_trait]
pub trait OrmpiConnectionTrait: Send + Sync {
    fn backend(&self) -> DbBackend;

    async fn execute_raw_sql(
        &self,
        sql: &str,
        params: &[OrmpiValue],
    ) -> crate::Result<u64>;

    async fn fetch_one_sql(
        &self,
        sql: &str,
        params: &[OrmpiValue],
    ) -> crate::Result<OrmpiRow>;

    async fn fetch_all_sql(
        &self,
        sql: &str,
        params: &[OrmpiValue],
    ) -> crate::Result<Vec<OrmpiRow>>;
}
```

Both `OrmpiPool` and `OrmpiTransaction` implement `OrmpiConnectionTrait`.

### 3.8 `OrmpiExecutor<'c>` — The Unification Enum

Mirrors SeaORM's `DatabaseExecutor<'c>`:

```rust
pub enum OrmpiExecutor<'c> {
    Pool(&'c OrmpiPool),
    Transaction(&'c OrmpiTransaction),
}

impl OrmpiConnectionTrait for OrmpiExecutor<'_> {
    fn backend(&self) -> DbBackend {
        match self {
            OrmpiExecutor::Pool(p) => p.backend(),
            OrmpiExecutor::Transaction(t) => t.backend,
        }
    }

    async fn execute_raw_sql(&self, sql: &str, params: &[OrmpiValue]) -> crate::Result<u64> {
        match self {
            OrmpiExecutor::Pool(p) => p.execute_raw_sql(sql, params).await,
            OrmpiExecutor::Transaction(t) => t.execute_raw_sql(sql, params).await,
        }
    }
    // ... same for fetch_one_sql, fetch_all_sql
}

// Conversions
impl<'c> From<&'c OrmpiPool> for OrmpiExecutor<'c> {
    fn from(p: &'c OrmpiPool) -> Self { OrmpiExecutor::Pool(p) }
}

impl<'c> From<&'c OrmpiTransaction> for OrmpiExecutor<'c> {
    fn from(t: &'c OrmpiTransaction) -> Self { OrmpiExecutor::Transaction(t) }
}
```

### 3.9 Update All Operation Functions

Change `&OrmpiPool` → `&impl OrmpiConnectionTrait`:

```rust
// Before
pub async fn save_instance(pool: &OrmpiPool, ...) -> Result<OrmpiValue>

// After
pub async fn save_instance(
    db: &impl OrmpiConnectionTrait,
    ...
) -> Result<OrmpiValue>
```

**No logic changes inside these functions** — only the executor type changes.
All the query building, binding, and result handling stays identical.

### 3.10 Closure-Based Transaction API

Mirrors SeaORM's `transaction()` method exactly:

```rust
impl OrmpiPool {
    pub async fn transaction<F, T, E>(&self, callback: F) -> Result<T, OrmpiTransactionError<E>>
    where
        F: for<'c> FnOnce(
            &'c OrmpiTransaction,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        let txn = OrmpiTransaction::begin(self, "default")
            .await
            .map_err(OrmpiTransactionError::Connection)?;

        let result = callback(&txn).await.map_err(OrmpiTransactionError::Transaction);

        if result.is_ok() {
            txn.commit().await.map_err(OrmpiTransactionError::Connection)?;
        } else {
            txn.rollback().await.map_err(OrmpiTransactionError::Connection)?;
        }

        result
    }
}

// Same impl on OrmpiTransaction — calls begin_nested() instead
impl OrmpiTransaction {
    pub async fn transaction<F, T, E>(&self, callback: F) -> Result<T, OrmpiTransactionError<E>>
    where ... {
        let nested = self.begin_nested().await.map_err(OrmpiTransactionError::Connection)?;
        let result = callback(&nested).await.map_err(OrmpiTransactionError::Transaction);
        if result.is_ok() {
            nested.commit().await.map_err(OrmpiTransactionError::Connection)?;
        } else {
            nested.rollback().await.map_err(OrmpiTransactionError::Connection)?;
        }
        result
    }
}
```

### 3.11 Python-Side Usage (After Adoption)

```python
# Closure style — recommended, matches SeaORM's .transaction()
async def transfer(from_id: int, to_id: int, amount: Decimal):
    async def body(txn):
        src = await Account.objects.select_for_update().aget(id=from_id, using=txn)
        dst = await Account.objects.aget(id=to_id, using=txn)
        src.balance -= amount
        dst.balance += amount
        await src.asave(using=txn)
        await dst.asave(using=txn)

    await db.transaction(body)

# Explicit style — for complex flows
async def complex_flow():
    txn = await db.begin()
    try:
        await Order.objects.acreate(using=txn, ...)
        await Payment.objects.acreate(using=txn, ...)
        await txn.commit()
    except Exception:
        await txn.rollback()
        raise
```

---

## Part 4: Side-by-Side Comparison

### Transaction Begin

| | SeaORM | ormpi (current) | ormpi (target) |
|---|--------|-----------------|----------------|
| Acquire | `pool.acquire()` → `PoolConnection` | `pool.begin()` → `Transaction<'static>` | `pool.acquire()` → `PoolConnection` |
| Drive BEGIN | `TransactionManager::begin()` on `&mut *conn` | sqlx internal | `TransactionManager::begin()` on `&mut *conn` |
| Unsafe | None | `unsafe transmute` | None |
| Storage | `Arc<Mutex<InnerConnection>>` | `Box<sqlx::Transaction<'static>>` | `Arc<Mutex<InnerConnection>>` |

### Nesting

| | SeaORM | ormpi (current) | ormpi (target) |
|---|--------|-----------------|----------------|
| Nested begin | `Arc::clone` → same conn | Not supported | `Arc::clone` → same conn |
| SQL issued | `SAVEPOINT` (auto via depth counter) | N/A | `SAVEPOINT` (auto via depth counter) |
| Implementation | SeaORM delegates depth tracking to sqlx | N/A | Same |

### Operation Call Sites

| | SeaORM | ormpi (current) | ormpi (target) |
|---|--------|-----------------|----------------|
| Parameter type | `impl ConnectionTrait` | `&OrmpiPool` | `impl OrmpiConnectionTrait` |
| Works with pool | ✅ | ✅ | ✅ |
| Works with transaction | ✅ | ❌ | ✅ |
| Signature change on child ops | None | N/A | Only parameter type changes |

### Drop / Safety

| | SeaORM | ormpi (current) | ormpi (target) |
|---|--------|-----------------|----------------|
| Uncommitted txn | Auto-rollback via `start_rollback` in Drop | Leaked (no Drop) | Auto-rollback via `start_rollback` in Drop |
| Mechanism | `try_lock()` → `TransactionManager::start_rollback()` | N/A | Same |

---

## Part 5: Files to Change in ormpi

```
ormpi-core/src/
├── transaction.rs          ← Full rewrite (InnerConnection, Arc<Mutex>, no unsafe, Drop)
├── connection.rs           ← New file: OrmpiConnectionTrait + OrmpiExecutor<'c>
├── pool.rs                 ← Add OrmpiConnectionTrait impl + transaction() method
├── model/executor.rs       ← Change &OrmpiPool → &impl OrmpiConnectionTrait in all fns
├── executor.rs             ← Change &OrmpiPool → &impl OrmpiConnectionTrait in all fns
└── lib.rs                  ← Re-export new types
```

### Changes ranked by complexity

| File | Change | Complexity |
|------|--------|------------|
| `transaction.rs` | Full rewrite — new struct, Arc/Mutex, Drop, nesting | High |
| `connection.rs` | New file — trait + executor enum | Medium |
| `pool.rs` | Add `OrmpiConnectionTrait` impl + `transaction()` closure API | Medium |
| `model/executor.rs` | Swap `&OrmpiPool` → `&impl OrmpiConnectionTrait` in 5 functions | Low |
| `executor.rs` | Swap `&OrmpiPool` → `&impl OrmpiConnectionTrait` in 3 functions | Low |

The operation functions (`save_instance`, `delete_instance`, etc.) contain
**zero transaction logic**. They only need their parameter type changed. All
query building stays identical.

---

## Part 6: What to Skip from SeaORM

| SeaORM feature | Reason to skip for now |
|----------------|------------------------|
| `IsolationLevel` / `AccessMode` | Add later — not needed for basic transaction support |
| `SqliteTransactionMode` | Add later — `DEFERRED` is fine as the default |
| `StreamTrait` | No streaming support in ormpi yet |
| `RestrictedConnection` (RBAC) | Different feature entirely |
| Mock backend | Has its own `MockDatabaseConnection` |
| Proxy backend | Has its own `ProxyDatabaseConnection` |
| `TransactionOptions` struct | Add with isolation levels — post-MVP |
| `metric_callback` | Add later for observability |
| `begin_with_config` / `begin_with_options` | Add with isolation levels |

The MVP is: `Arc<Mutex<InnerConnection>>`, `OrmpiConnectionTrait`, `OrmpiExecutor`,
updated operation signatures, `Drop` rollback. Everything else is additive.
