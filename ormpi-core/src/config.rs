use std::collections::HashMap;
use std::time::Duration;

use crate::backend::DatabaseBackend;
use crate::error::OrmpiError;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub alias: String,
    pub backend: DatabaseBackend,
    /// Database name or SQLite file path.
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub pool: PoolConfig,
    pub test_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
    pub acquire_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: None,
            max_lifetime: None,
            acquire_timeout: Duration::from_secs(30),
        }
    }
}

impl DatabaseConfig {
    /// Build connection URL string.
    pub fn connection_url(&self) -> String {
        match self.backend {
            DatabaseBackend::Sqlite => {
                if self.name == ":memory:" {
                    "sqlite::memory:".to_string()
                } else {
                    format!("sqlite://{}", self.name)
                }
            }
            DatabaseBackend::Mysql => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.name
            ),
            DatabaseBackend::Postgres => format!(
                "postgres://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.name
            ),
        }
    }

    /// Parse from a Python dict-equivalent HashMap.
    pub fn from_map(alias: &str, map: &HashMap<String, serde_json::Value>) -> crate::Result<Self> {
        let engine = map
            .get("ENGINE")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OrmpiError::ConfigError("ENGINE is required".into()))?;
        let backend = DatabaseBackend::from_engine(engine)?;

        let default_port = match backend {
            DatabaseBackend::Sqlite => 0,
            DatabaseBackend::Mysql => 3306,
            DatabaseBackend::Postgres => 5432,
        };

        Ok(DatabaseConfig {
            alias: alias.to_string(),
            backend,
            name: str_field(map, "NAME")?,
            host: str_field_or(map, "HOST", "localhost"),
            port: u16_field_or(map, "PORT", default_port),
            user: str_field_or(map, "USER", ""),
            password: str_field_or(map, "PASSWORD", ""),
            pool: parse_pool_options(map),
            test_name: map
                .get("TEST")
                .and_then(|v| v.get("NAME"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
        })
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn str_field(map: &HashMap<String, serde_json::Value>, key: &str) -> crate::Result<String> {
    map.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| OrmpiError::ConfigError(format!("{key} is required")))
}

fn str_field_or(map: &HashMap<String, serde_json::Value>, key: &str, default: &str) -> String {
    map.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

fn u16_field_or(map: &HashMap<String, serde_json::Value>, key: &str, default: u16) -> u16 {
    map.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u16)
        .unwrap_or(default)
}

fn parse_pool_options(map: &HashMap<String, serde_json::Value>) -> PoolConfig {
    let mut cfg = PoolConfig::default();
    if let Some(v) = map.get("POOL_MAX_CONNECTIONS").and_then(|v| v.as_u64()) {
        cfg.max_connections = v as u32;
    }
    if let Some(v) = map.get("POOL_MIN_CONNECTIONS").and_then(|v| v.as_u64()) {
        cfg.min_connections = v as u32;
    }
    if let Some(v) = map.get("POOL_CONNECT_TIMEOUT_SECS").and_then(|v| v.as_u64()) {
        cfg.connect_timeout = Duration::from_secs(v);
    }
    if let Some(v) = map.get("POOL_ACQUIRE_TIMEOUT_SECS").and_then(|v| v.as_u64()) {
        cfg.acquire_timeout = Duration::from_secs(v);
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn test_sqlite_config() {
        let m = map(&[
            ("ENGINE", json!("sqlite")),
            ("NAME", json!(":memory:")),
        ]);
        let cfg = DatabaseConfig::from_map("default", &m).unwrap();
        assert_eq!(cfg.backend, DatabaseBackend::Sqlite);
        assert_eq!(cfg.connection_url(), "sqlite::memory:");
    }

    #[test]
    fn test_postgres_defaults() {
        let m = map(&[
            ("ENGINE", json!("postgres")),
            ("NAME", json!("mydb")),
        ]);
        let cfg = DatabaseConfig::from_map("db", &m).unwrap();
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.host, "localhost");
    }

    #[test]
    fn test_missing_engine() {
        let m: HashMap<String, serde_json::Value> = HashMap::new();
        assert!(DatabaseConfig::from_map("default", &m).is_err());
    }

    #[test]
    fn test_missing_name() {
        let m = map(&[("ENGINE", json!("sqlite"))]);
        assert!(DatabaseConfig::from_map("default", &m).is_err());
    }
}
