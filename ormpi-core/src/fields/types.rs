/// All supported database field/column types.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    // Auto primary keys
    AutoInt,
    AutoBigInt,
    AutoSmallInt,

    // Integer types
    SmallInt,
    Int,
    BigInt,
    PositiveSmallInt,
    PositiveInt,
    PositiveBigInt,

    // Float / Decimal
    Float,
    Double,
    Decimal { max_digits: u32, decimal_places: u32 },

    // String types
    Char { max_length: u32 },
    Text,
    Slug { max_length: u32 },
    Email { max_length: u32 },
    Url { max_length: u32 },
    IpAddress,
    GenericIpAddress,

    // Boolean
    Boolean,

    // Temporal
    Date,
    Time,
    DateTime,
    Duration,

    // Binary / special
    Binary,
    Uuid,
    Json,
    JsonB, // Postgres-specific

    // Relational (resolved separately — don't map to a single column)
    ForeignKey {
        to: String,
        on_delete: OnDelete,
        db_column: Option<String>,
    },
    OneToOne {
        to: String,
        on_delete: OnDelete,
    },
    ManyToMany {
        to: String,
        through: Option<String>,
        db_table: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDelete {
    Cascade,
    Protect,
    SetNull,
    SetDefault,
    DoNothing,
    Restrict,
}
