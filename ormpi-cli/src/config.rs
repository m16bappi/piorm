use ormpi_core::backend::DatabaseBackend;
use ormpi_core::config::{DatabaseConfig, PoolConfig};
use ormpi_core::OrmpiError;

/// Parse a database URL string into a `DatabaseConfig`.
///
/// Supported formats:
/// - `sqlite::memory:`
/// - `sqlite://./path/to/db.sqlite3`
/// - `postgres://user:pass@host:5432/dbname`
/// - `postgresql://user:pass@host:5432/dbname`
/// - `mysql://user:pass@host:3306/dbname`
pub fn parse_database_url(alias: &str, url: &str) -> ormpi_core::Result<DatabaseConfig> {
    if url == "sqlite::memory:" || url == "sqlite://:memory:" {
        return Ok(make_sqlite_config(alias, ":memory:"));
    }

    if let Some(path) = url.strip_prefix("sqlite://") {
        return Ok(make_sqlite_config(alias, path));
    }

    let (backend, rest) = if let Some(r) = url.strip_prefix("postgresql://") {
        (DatabaseBackend::Postgres, r)
    } else if let Some(r) = url.strip_prefix("postgres://") {
        (DatabaseBackend::Postgres, r)
    } else if let Some(r) = url.strip_prefix("mysql://") {
        (DatabaseBackend::Mysql, r)
    } else {
        return Err(OrmpiError::ConfigError(format!(
            "Unknown database URL scheme: {url}"
        )));
    };

    let (userinfo, hostdb) = rest.split_once('@').ok_or_else(|| {
        OrmpiError::ConfigError(format!("Invalid database URL (missing @): {url}"))
    })?;

    let (user, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));

    let (hostport, dbname) = hostdb.split_once('/').ok_or_else(|| {
        OrmpiError::ConfigError(format!("Invalid database URL (missing /dbname): {url}"))
    })?;

    let default_port = match backend {
        DatabaseBackend::Postgres => 5432,
        DatabaseBackend::Mysql => 3306,
        DatabaseBackend::Sqlite => 0,
    };

    let (host, port) = if let Some((h, p)) = hostport.split_once(':') {
        let port: u16 = p
            .parse()
            .map_err(|_| OrmpiError::ConfigError(format!("Invalid port in URL: {url}")))?;
        (h, port)
    } else {
        (hostport, default_port)
    };

    Ok(DatabaseConfig {
        alias: alias.to_string(),
        backend,
        name: dbname.to_string(),
        host: host.to_string(),
        port,
        user: user.to_string(),
        password: password.to_string(),
        pool: PoolConfig::default(),
        test_name: None,
    })
}

fn make_sqlite_config(alias: &str, name: &str) -> DatabaseConfig {
    DatabaseConfig {
        alias: alias.to_string(),
        backend: DatabaseBackend::Sqlite,
        name: name.to_string(),
        host: String::new(),
        port: 0,
        user: String::new(),
        password: String::new(),
        pool: PoolConfig::default(),
        test_name: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sqlite_memory() {
        let cfg = parse_database_url("default", "sqlite::memory:").unwrap();
        assert_eq!(cfg.backend, DatabaseBackend::Sqlite);
        assert_eq!(cfg.name, ":memory:");
    }

    #[test]
    fn test_parse_sqlite_file() {
        let cfg = parse_database_url("default", "sqlite://./db.sqlite3").unwrap();
        assert_eq!(cfg.backend, DatabaseBackend::Sqlite);
        assert_eq!(cfg.name, "./db.sqlite3");
    }

    #[test]
    fn test_parse_postgres() {
        let cfg =
            parse_database_url("default", "postgres://admin:secret@localhost:5432/mydb").unwrap();
        assert_eq!(cfg.backend, DatabaseBackend::Postgres);
        assert_eq!(cfg.user, "admin");
        assert_eq!(cfg.password, "secret");
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.name, "mydb");
    }

    #[test]
    fn test_parse_unknown_scheme() {
        assert!(parse_database_url("default", "redis://localhost").is_err());
    }
}
