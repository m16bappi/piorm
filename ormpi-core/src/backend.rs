use sea_query::{
    DeleteStatement, IndexCreateStatement, InsertStatement, MysqlQueryBuilder,
    PostgresQueryBuilder, SelectStatement, SqliteQueryBuilder, TableAlterStatement,
    TableCreateStatement, TableDropStatement, UpdateStatement, Values,
};

/// The three supported SQL dialects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseBackend {
    Sqlite,
    Mysql,
    Postgres,
}

impl DatabaseBackend {
    /// Parse from ENGINE string (supports multiple aliases).
    pub fn from_engine(engine: &str) -> crate::Result<Self> {
        match engine.to_lowercase().as_str() {
            "sqlite" | "sqlite3" | "ormpi.backends.sqlite" => Ok(Self::Sqlite),
            "mysql" | "mariadb" | "ormpi.backends.mysql" => Ok(Self::Mysql),
            "postgres" | "postgresql" | "ormpi.backends.postgres" | "ormpi.backends.postgresql" => {
                Ok(Self::Postgres)
            }
            other => Err(crate::OrmpiError::UnknownBackend(other.to_string())),
        }
    }

    /// Returns the placeholder style used in parameterized queries.
    /// SQLite/MySQL use `?`, Postgres uses `$1, $2, ...`
    pub fn placeholder_style(&self) -> PlaceholderStyle {
        match self {
            Self::Postgres => PlaceholderStyle::Numbered,
            _ => PlaceholderStyle::QuestionMark,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PlaceholderStyle {
    QuestionMark, // ? (MySQL, SQLite)
    Numbered,     // $1, $2, $3 (Postgres)
}

/// Wraps sea-query builders to provide a unified interface.
#[derive(Debug, Clone, Copy)]
pub struct QueryBackend(pub DatabaseBackend);

impl QueryBackend {
    pub fn build_select(&self, stmt: &SelectStatement) -> (String, Values) {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.build(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.build(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.build(PostgresQueryBuilder),
        }
    }

    pub fn build_insert(&self, stmt: &InsertStatement) -> (String, Values) {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.build(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.build(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.build(PostgresQueryBuilder),
        }
    }

    pub fn build_update(&self, stmt: &UpdateStatement) -> (String, Values) {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.build(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.build(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.build(PostgresQueryBuilder),
        }
    }

    pub fn build_delete(&self, stmt: &DeleteStatement) -> (String, Values) {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.build(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.build(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.build(PostgresQueryBuilder),
        }
    }

    pub fn build_table_create(&self, stmt: &TableCreateStatement) -> String {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.to_string(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
        }
    }

    pub fn build_table_alter(&self, stmt: &TableAlterStatement) -> String {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.to_string(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
        }
    }

    pub fn build_table_drop(&self, stmt: &TableDropStatement) -> String {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.to_string(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
        }
    }

    pub fn build_index_create(&self, stmt: &IndexCreateStatement) -> String {
        match self.0 {
            DatabaseBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
            DatabaseBackend::Mysql => stmt.to_string(MysqlQueryBuilder),
            DatabaseBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_query::{Alias, Expr, ExprTrait, Query};

    #[test]
    fn test_sqlite_select() {
        let (sql, _values) = QueryBackend(DatabaseBackend::Sqlite).build_select(
            &Query::select()
                .column(sea_query::Asterisk)
                .from(Alias::new("users"))
                .and_where(Expr::col(Alias::new("id")).eq(1))
                .to_owned(),
        );
        assert!(sql.contains("WHERE"), "expected WHERE in: {sql}");
        assert!(sql.contains('"'), "SQLite should double-quote identifiers, got: {sql}");
    }

    #[test]
    fn test_from_engine_aliases() {
        assert_eq!(DatabaseBackend::from_engine("sqlite").unwrap(), DatabaseBackend::Sqlite);
        assert_eq!(
            DatabaseBackend::from_engine("POSTGRES").unwrap(),
            DatabaseBackend::Postgres
        );
        assert_eq!(DatabaseBackend::from_engine("mariadb").unwrap(), DatabaseBackend::Mysql);
        assert!(DatabaseBackend::from_engine("oracle").is_err());
    }
}
