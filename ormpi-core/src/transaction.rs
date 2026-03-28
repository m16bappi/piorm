use crate::error::OrmpiError;
use crate::pool::OrmpiPool;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionState {
    Open,
    Committed,
    RolledBack,
}

pub enum TransactionHandle {
    Sqlite(Box<sqlx::Transaction<'static, sqlx::Sqlite>>),
    Mysql(Box<sqlx::Transaction<'static, sqlx::MySql>>),
    Postgres(Box<sqlx::Transaction<'static, sqlx::Postgres>>),
}

pub struct OrmpiTransaction {
    pub handle: TransactionHandle,
    pub alias: String,
    pub state: TransactionState,
}

impl OrmpiTransaction {
    /// Begin a new transaction on the given pool.
    ///
    /// # Safety
    /// The pool lives in the global registry for the process lifetime, so the
    /// transmuted `'static` bound on the transaction is sound.
    pub async fn begin(pool: &OrmpiPool, alias: &str) -> crate::Result<Self> {
        let handle = match pool {
            OrmpiPool::Sqlite(p) => {
                let txn = p.begin().await?;
                // SAFETY: Pool lives in the global registry (process lifetime).
                let txn: sqlx::Transaction<'static, sqlx::Sqlite> =
                    unsafe { std::mem::transmute(txn) };
                TransactionHandle::Sqlite(Box::new(txn))
            }
            OrmpiPool::Mysql(p) => {
                let txn = p.begin().await?;
                let txn: sqlx::Transaction<'static, sqlx::MySql> =
                    unsafe { std::mem::transmute(txn) };
                TransactionHandle::Mysql(Box::new(txn))
            }
            OrmpiPool::Postgres(p) => {
                let txn = p.begin().await?;
                let txn: sqlx::Transaction<'static, sqlx::Postgres> =
                    unsafe { std::mem::transmute(txn) };
                TransactionHandle::Postgres(Box::new(txn))
            }
        };
        Ok(OrmpiTransaction {
            handle,
            alias: alias.to_string(),
            state: TransactionState::Open,
        })
    }

    /// Commit the transaction, consuming it.
    pub async fn commit(self) -> crate::Result<()> {
        if self.state != TransactionState::Open {
            return Err(OrmpiError::TransactionError(
                "Transaction is not open".into(),
            ));
        }
        match self.handle {
            TransactionHandle::Sqlite(boxed) => (*boxed).commit().await?,
            TransactionHandle::Mysql(boxed) => (*boxed).commit().await?,
            TransactionHandle::Postgres(boxed) => (*boxed).commit().await?,
        }
        Ok(())
    }

    /// Roll back the transaction, consuming it.
    pub async fn rollback(self) -> crate::Result<()> {
        if self.state != TransactionState::Open {
            return Err(OrmpiError::TransactionError(
                "Transaction is not open".into(),
            ));
        }
        match self.handle {
            TransactionHandle::Sqlite(boxed) => (*boxed).rollback().await?,
            TransactionHandle::Mysql(boxed) => (*boxed).rollback().await?,
            TransactionHandle::Postgres(boxed) => (*boxed).rollback().await?,
        }
        Ok(())
    }

    /// Execute raw SQL on this transaction's connection (e.g. for savepoints).
    pub async fn execute_raw(&mut self, sql: &str) -> crate::Result<()> {
        use std::ops::DerefMut;
        match &mut self.handle {
            TransactionHandle::Sqlite(t) => {
                sqlx::query(sql).execute(t.as_mut().deref_mut()).await?;
            }
            TransactionHandle::Mysql(t) => {
                sqlx::query(sql).execute(t.as_mut().deref_mut()).await?;
            }
            TransactionHandle::Postgres(t) => {
                sqlx::query(sql).execute(t.as_mut().deref_mut()).await?;
            }
        }
        Ok(())
    }
}
