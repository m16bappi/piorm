use sqlx::{MySql, Pool, Postgres, Sqlite};

use crate::backend::DatabaseBackend;
use crate::config::DatabaseConfig;

#[derive(Clone)]
pub enum OrmpiPool {
    Sqlite(Pool<Sqlite>),
    Mysql(Pool<MySql>),
    Postgres(Pool<Postgres>),
}

impl OrmpiPool {
    /// Create a pool from a `DatabaseConfig`.
    pub async fn connect(config: &DatabaseConfig) -> crate::Result<Self> {
        let url = config.connection_url();
        let opts = &config.pool;

        match config.backend {
            DatabaseBackend::Sqlite => {
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(opts.max_connections)
                    .min_connections(opts.min_connections)
                    .acquire_timeout(opts.acquire_timeout)
                    .connect(&url)
                    .await?;
                Ok(OrmpiPool::Sqlite(pool))
            }
            DatabaseBackend::Mysql => {
                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(opts.max_connections)
                    .min_connections(opts.min_connections)
                    .acquire_timeout(opts.acquire_timeout)
                    .connect(&url)
                    .await?;
                Ok(OrmpiPool::Mysql(pool))
            }
            DatabaseBackend::Postgres => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(opts.max_connections)
                    .min_connections(opts.min_connections)
                    .acquire_timeout(opts.acquire_timeout)
                    .connect(&url)
                    .await?;
                Ok(OrmpiPool::Postgres(pool))
            }
        }
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
