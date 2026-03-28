use thiserror::Error;

pub type Result<T> = std::result::Result<T, OrmpiError>;

#[derive(Debug, Error)]
pub enum OrmpiError {
    // Configuration
    #[error("Unknown database backend: {0}")]
    UnknownBackend(String),

    #[error("Unknown database alias: {0}")]
    UnknownDatabase(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    // Database / sqlx errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Database connection failed: {0}")]
    ConnectionFailed(String),

    // ORM errors
    #[error("{model}.DoesNotExist: {model} matching query does not exist.")]
    DoesNotExist { model: String },

    #[error("get() returned more than one {model}: {count} objects returned")]
    MultipleObjectsReturned { model: String, count: usize },

    #[error("Field error on {model}.{field}: {message}")]
    FieldError {
        model: String,
        field: String,
        message: String,
    },

    #[error("Validation error: {0}")]
    ValidationError(String),

    // Transaction errors
    #[error("Transaction error: {0}")]
    TransactionError(String),

    // Migration errors
    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("Migration conflict: {0}")]
    MigrationConflict(String),

    // Query errors
    #[error("Invalid field lookup: {0}")]
    InvalidLookup(String),

    #[error("Cannot resolve keyword '{keyword}' into field. Choices are: {choices}")]
    UnknownField { keyword: String, choices: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
