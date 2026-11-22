use sea_query::Values;
use sqlx::{MySql, Pool, Postgres, Sqlite};

use crate::backend::DatabaseBackend;
use crate::bind::{mysql_arguments, postgres_arguments, sqlite_arguments};
use crate::config::DatabaseConfig;
use crate::row::{mysql_row_to_piorm, postgres_row_to_piorm, sqlite_row_to_piorm, PiormRow};

/// Build a sqlx pool with the standard Piorm options and wrap it in the given `PiormPool` variant.
///
/// Usage: `connect_pool!(SqlitePoolOptions, Sqlite, &url, opts).await?`
macro_rules! connect_pool {
    ($options_ty:path, $variant:ident, $url:expr, $opts:expr) => {
        <$options_ty>::new()
            .max_connections($opts.max_connections)
            .min_connections($opts.min_connections)
            .acquire_timeout($opts.acquire_timeout)
            .connect($url)
            .await
            .map(PiormPool::$variant)
    };
}

#[derive(Clone)]
pub enum PiormPool {
    Sqlite(Pool<Sqlite>),
    Mysql(Pool<MySql>),
    Postgres(Pool<Postgres>),
}

impl PiormPool {
    /// Create a pool from a `DatabaseConfig`.
    pub async fn connect(config: &DatabaseConfig) -> crate::Result<Self> {
        use sqlx::{mysql::MySqlPoolOptions, postgres::PgPoolOptions, sqlite::SqlitePoolOptions};

        let url = config.connection_url();
        let opts = &config.pool;

        let pool = match config.backend {
            DatabaseBackend::Sqlite   => connect_pool!(SqlitePoolOptions, Sqlite,   &url, opts)?,
            DatabaseBackend::Mysql    => connect_pool!(MySqlPoolOptions,  Mysql,    &url, opts)?,
            DatabaseBackend::Postgres => connect_pool!(PgPoolOptions,     Postgres, &url, opts)?,
        };
        Ok(pool)
    }

    pub fn backend(&self) -> DatabaseBackend {
        match self {
            PiormPool::Sqlite(_) => DatabaseBackend::Sqlite,
            PiormPool::Mysql(_) => DatabaseBackend::Mysql,
            PiormPool::Postgres(_) => DatabaseBackend::Postgres,
        }
    }

    pub async fn close(&self) {
        match self {
            PiormPool::Sqlite(p) => p.close().await,
            PiormPool::Mysql(p) => p.close().await,
            PiormPool::Postgres(p) => p.close().await,
        }
    }

    /// Execute a DML/DDL statement and return the number of rows affected.
    pub async fn execute(&self, sql: &str, values: Values) -> crate::Result<u64> {
        match self {
            PiormPool::Sqlite(p) => Ok(sqlx::query_with(sql, sqlite_arguments(values))
                .execute(p).await?.rows_affected()),
            PiormPool::Mysql(p) => Ok(sqlx::query_with(sql, mysql_arguments(values))
                .execute(p).await?.rows_affected()),
            PiormPool::Postgres(p) => Ok(sqlx::query_with(sql, postgres_arguments(values))
                .execute(p).await?.rows_affected()),
        }
    }

    /// Execute an INSERT and return the last inserted row id (SQLite/MySQL only).
    /// For Postgres, build the statement with `RETURNING` and use `fetch_one` instead.
    pub async fn insert_last_id(&self, sql: &str, values: Values) -> crate::Result<i64> {
        match self {
            PiormPool::Sqlite(p) => Ok(sqlx::query_with(sql, sqlite_arguments(values))
                .execute(p).await?.last_insert_rowid()),
            PiormPool::Mysql(p) => Ok(sqlx::query_with(sql, mysql_arguments(values))
                .execute(p).await?.last_insert_id() as i64),
            PiormPool::Postgres(_) => Err(crate::PiormError::Other(
                "insert_last_id is not supported for Postgres; use fetch_one with RETURNING".into(),
            )),
        }
    }

    /// Execute a SELECT and return all rows as `PiormRow`.
    pub async fn fetch_all(&self, sql: &str, values: Values) -> crate::Result<Vec<PiormRow>> {
        match self {
            PiormPool::Sqlite(p) => sqlx::query_with(sql, sqlite_arguments(values))
                .fetch_all(p).await?
                .iter().map(sqlite_row_to_piorm).collect(),
            PiormPool::Mysql(p) => sqlx::query_with(sql, mysql_arguments(values))
                .fetch_all(p).await?
                .iter().map(mysql_row_to_piorm).collect(),
            PiormPool::Postgres(p) => sqlx::query_with(sql, postgres_arguments(values))
                .fetch_all(p).await?
                .iter().map(postgres_row_to_piorm).collect(),
        }
    }

    /// Execute a SELECT and return the first row. Propagates `RowNotFound` as-is.
    pub async fn fetch_one(&self, sql: &str, values: Values) -> crate::Result<PiormRow> {
        match self {
            PiormPool::Sqlite(p) => sqlite_row_to_piorm(
                &sqlx::query_with(sql, sqlite_arguments(values)).fetch_one(p).await?),
            PiormPool::Mysql(p) => mysql_row_to_piorm(
                &sqlx::query_with(sql, mysql_arguments(values)).fetch_one(p).await?),
            PiormPool::Postgres(p) => postgres_row_to_piorm(
                &sqlx::query_with(sql, postgres_arguments(values)).fetch_one(p).await?),
        }
    }

    /// Execute a SELECT and return the first row or `None`.
    pub async fn fetch_optional(&self, sql: &str, values: Values) -> crate::Result<Option<PiormRow>> {
        match self {
            PiormPool::Sqlite(p) => sqlx::query_with(sql, sqlite_arguments(values))
                .fetch_optional(p).await?
                .map(|r| sqlite_row_to_piorm(&r))
                .transpose(),
            PiormPool::Mysql(p) => sqlx::query_with(sql, mysql_arguments(values))
                .fetch_optional(p).await?
                .map(|r| mysql_row_to_piorm(&r))
                .transpose(),
            PiormPool::Postgres(p) => sqlx::query_with(sql, postgres_arguments(values))
                .fetch_optional(p).await?
                .map(|r| postgres_row_to_piorm(&r))
                .transpose(),
        }
    }
}

impl std::fmt::Debug for PiormPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(_) => write!(f, "PiormPool::Sqlite(...)"),
            Self::Mysql(_) => write!(f, "PiormPool::Mysql(...)"),
            Self::Postgres(_) => write!(f, "PiormPool::Postgres(...)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PoolConfig;

    #[tokio::test]
    async fn test_sqlite_in_memory_pool() {
        let config = DatabaseConfig {
            alias: "default".into(),
            backend: DatabaseBackend::Sqlite,
            name: ":memory:".into(),
            host: "localhost".into(),
            port: 0,
            user: "".into(),
            password: "".into(),
            pool: PoolConfig::default(),
            test_name: None,
        };
        let pool = PiormPool::connect(&config).await.expect("pool connect");
        assert_eq!(pool.backend(), DatabaseBackend::Sqlite);
        pool.close().await;
    }
}
