use sea_query::Values;
use sqlx::{MySql, Pool, Postgres, Sqlite};

use crate::backend::DatabaseBackend;
use crate::bind::{mysql_arguments, postgres_arguments, sqlite_arguments};
use crate::config::DatabaseConfig;
use crate::row::{mysql_row_to_ormpi, postgres_row_to_ormpi, sqlite_row_to_ormpi, OrmpiRow};

/// Build a sqlx pool with the standard Ormpi options and wrap it in the given `OrmpiPool` variant.
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
            .map(OrmpiPool::$variant)
    };
}

#[derive(Clone)]
pub enum OrmpiPool {
    Sqlite(Pool<Sqlite>),
    Mysql(Pool<MySql>),
    Postgres(Pool<Postgres>),
}

impl OrmpiPool {
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
            OrmpiPool::Sqlite(_) => DatabaseBackend::Sqlite,
            OrmpiPool::Mysql(_) => DatabaseBackend::Mysql,
            OrmpiPool::Postgres(_) => DatabaseBackend::Postgres,
        }
    }

    pub async fn close(&self) {
        match self {
            OrmpiPool::Sqlite(p) => p.close().await,
            OrmpiPool::Mysql(p) => p.close().await,
            OrmpiPool::Postgres(p) => p.close().await,
        }
    }

    /// Execute a DML/DDL statement and return the number of rows affected.
    pub async fn execute(&self, sql: &str, values: Values) -> crate::Result<u64> {
        match self {
            OrmpiPool::Sqlite(p) => Ok(sqlx::query_with(sql, sqlite_arguments(values))
                .execute(p).await?.rows_affected()),
            OrmpiPool::Mysql(p) => Ok(sqlx::query_with(sql, mysql_arguments(values))
                .execute(p).await?.rows_affected()),
            OrmpiPool::Postgres(p) => Ok(sqlx::query_with(sql, postgres_arguments(values))
                .execute(p).await?.rows_affected()),
        }
    }

    /// Execute an INSERT and return the last inserted row id (SQLite/MySQL only).
    /// For Postgres, build the statement with `RETURNING` and use `fetch_one` instead.
    pub async fn insert_last_id(&self, sql: &str, values: Values) -> crate::Result<i64> {
        match self {
            OrmpiPool::Sqlite(p) => Ok(sqlx::query_with(sql, sqlite_arguments(values))
                .execute(p).await?.last_insert_rowid()),
            OrmpiPool::Mysql(p) => Ok(sqlx::query_with(sql, mysql_arguments(values))
                .execute(p).await?.last_insert_id() as i64),
            OrmpiPool::Postgres(_) => Err(crate::OrmpiError::Other(
                "insert_last_id is not supported for Postgres; use fetch_one with RETURNING".into(),
            )),
        }
    }

    /// Execute a SELECT and return all rows as `OrmpiRow`.
    pub async fn fetch_all(&self, sql: &str, values: Values) -> crate::Result<Vec<OrmpiRow>> {
        match self {
            OrmpiPool::Sqlite(p) => sqlx::query_with(sql, sqlite_arguments(values))
                .fetch_all(p).await?
                .iter().map(sqlite_row_to_ormpi).collect(),
            OrmpiPool::Mysql(p) => sqlx::query_with(sql, mysql_arguments(values))
                .fetch_all(p).await?
                .iter().map(mysql_row_to_ormpi).collect(),
            OrmpiPool::Postgres(p) => sqlx::query_with(sql, postgres_arguments(values))
                .fetch_all(p).await?
                .iter().map(postgres_row_to_ormpi).collect(),
        }
    }

    /// Execute a SELECT and return the first row. Propagates `RowNotFound` as-is.
    pub async fn fetch_one(&self, sql: &str, values: Values) -> crate::Result<OrmpiRow> {
        match self {
            OrmpiPool::Sqlite(p) => sqlite_row_to_ormpi(
                &sqlx::query_with(sql, sqlite_arguments(values)).fetch_one(p).await?),
            OrmpiPool::Mysql(p) => mysql_row_to_ormpi(
                &sqlx::query_with(sql, mysql_arguments(values)).fetch_one(p).await?),
            OrmpiPool::Postgres(p) => postgres_row_to_ormpi(
                &sqlx::query_with(sql, postgres_arguments(values)).fetch_one(p).await?),
        }
    }

    /// Execute a SELECT and return the first row or `None`.
    pub async fn fetch_optional(&self, sql: &str, values: Values) -> crate::Result<Option<OrmpiRow>> {
        match self {
            OrmpiPool::Sqlite(p) => sqlx::query_with(sql, sqlite_arguments(values))
                .fetch_optional(p).await?
                .map(|r| sqlite_row_to_ormpi(&r))
                .transpose(),
            OrmpiPool::Mysql(p) => sqlx::query_with(sql, mysql_arguments(values))
                .fetch_optional(p).await?
                .map(|r| mysql_row_to_ormpi(&r))
                .transpose(),
            OrmpiPool::Postgres(p) => sqlx::query_with(sql, postgres_arguments(values))
                .fetch_optional(p).await?
                .map(|r| postgres_row_to_ormpi(&r))
                .transpose(),
        }
    }
}

impl std::fmt::Debug for OrmpiPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(_) => write!(f, "OrmpiPool::Sqlite(...)"),
            Self::Mysql(_) => write!(f, "OrmpiPool::Mysql(...)"),
            Self::Postgres(_) => write!(f, "OrmpiPool::Postgres(...)"),
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
        let pool = OrmpiPool::connect(&config).await.expect("pool connect");
        assert_eq!(pool.backend(), DatabaseBackend::Sqlite);
        pool.close().await;
    }
}
