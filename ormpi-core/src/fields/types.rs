/// All supported database field/column types.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
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

impl FieldType {
    /// Produce a stable string for change-detection comparison.
    pub fn to_type_str(&self) -> String {
        match self {
            FieldType::SmallInt         => "SmallIntField".to_string(),
            FieldType::Int              => "IntegerField".to_string(),
            FieldType::BigInt           => "BigIntegerField".to_string(),
            FieldType::PositiveSmallInt => "PositiveSmallIntegerField".to_string(),
            FieldType::PositiveInt      => "PositiveIntegerField".to_string(),
            FieldType::PositiveBigInt   => "PositiveBigIntegerField".to_string(),
            FieldType::Float            => "FloatField".to_string(),
            FieldType::Double           => "DoubleField".to_string(),
            FieldType::Decimal { max_digits, decimal_places } =>
                format!("DecimalField(max_digits={max_digits},decimal_places={decimal_places})"),
            FieldType::Char { max_length } => format!("CharField(max_length={max_length})"),
            FieldType::Text             => "TextField".to_string(),
            FieldType::Slug { max_length } => format!("SlugField(max_length={max_length})"),
            FieldType::Email { max_length } => format!("EmailField(max_length={max_length})"),
            FieldType::Url { max_length }   => format!("UrlField(max_length={max_length})"),
            FieldType::IpAddress        => "IPAddressField".to_string(),
            FieldType::GenericIpAddress => "GenericIPAddressField".to_string(),
            FieldType::Boolean          => "BooleanField".to_string(),
            FieldType::Date             => "DateField".to_string(),
            FieldType::Time             => "TimeField".to_string(),
            FieldType::DateTime         => "DateTimeField".to_string(),
            FieldType::Duration         => "DurationField".to_string(),
            FieldType::Binary           => "BinaryField".to_string(),
            FieldType::Uuid             => "UUIDField".to_string(),
            FieldType::Json             => "JSONField".to_string(),
            FieldType::JsonB            => "JSONBField".to_string(),
            FieldType::ForeignKey { to, on_delete, .. } =>
                format!("ForeignKey(to={to},on_delete={on_delete:?})"),
            FieldType::OneToOne { to, on_delete } =>
                format!("OneToOneField(to={to},on_delete={on_delete:?})"),
            FieldType::ManyToMany { to, .. } =>
                format!("ManyToManyField(to={to})"),
        }
    }

    /// Parse a `to_type_str()` representation back to a `FieldType`.
    /// Returns `None` for unrecognized strings.
    pub fn from_type_str(s: &str) -> Option<Self> {
        // Helpers for extracting simple key=value inside parens
        fn extract_u32(s: &str, key: &str) -> Option<u32> {
            let needle = format!("{}=", key);
            let start = s.find(&needle)? + needle.len();
            let end = s[start..].find([',', ')']).map(|i| start + i).unwrap_or(s.len());
            s[start..end].parse().ok()
        }
        fn extract_str<'a>(s: &'a str, key: &str) -> Option<&'a str> {
            let needle = format!("{}=", key);
            let start = s.find(&needle)? + needle.len();
            let end = s[start..].find([',', ')']).map(|i| start + i).unwrap_or(s.len());
            Some(&s[start..end])
        }

        match s {
            // Backward-compat: old migration files may contain these type strings.
            "AutoIntField"             => Some(FieldType::Int),
            "AutoBigIntField"          => Some(FieldType::BigInt),
            "AutoSmallIntField"        => Some(FieldType::SmallInt),
            "SmallIntField"            => Some(FieldType::SmallInt),
            "IntegerField"             => Some(FieldType::Int),
            "BigIntegerField"          => Some(FieldType::BigInt),
            "PositiveSmallIntegerField"=> Some(FieldType::PositiveSmallInt),
            "PositiveIntegerField"     => Some(FieldType::PositiveInt),
            "PositiveBigIntegerField"  => Some(FieldType::PositiveBigInt),
            "FloatField"               => Some(FieldType::Float),
            "DoubleField"              => Some(FieldType::Double),
            "TextField"                => Some(FieldType::Text),
            "IPAddressField"           => Some(FieldType::IpAddress),
            "GenericIPAddressField"    => Some(FieldType::GenericIpAddress),
            "BooleanField"             => Some(FieldType::Boolean),
            "DateField"                => Some(FieldType::Date),
            "TimeField"                => Some(FieldType::Time),
            "DateTimeField"            => Some(FieldType::DateTime),
            "DurationField"            => Some(FieldType::Duration),
            "BinaryField"              => Some(FieldType::Binary),
            "UUIDField"                => Some(FieldType::Uuid),
            "JSONField"                => Some(FieldType::Json),
            "JSONBField"               => Some(FieldType::JsonB),
            _ if s.starts_with("CharField(")  => extract_u32(s, "max_length").map(|ml| FieldType::Char  { max_length: ml }),
            _ if s.starts_with("SlugField(")  => extract_u32(s, "max_length").map(|ml| FieldType::Slug  { max_length: ml }),
            _ if s.starts_with("EmailField(") => extract_u32(s, "max_length").map(|ml| FieldType::Email { max_length: ml }),
            _ if s.starts_with("UrlField(")   => extract_u32(s, "max_length").map(|ml| FieldType::Url   { max_length: ml }),
            _ if s.starts_with("DecimalField(") => {
                let md = extract_u32(s, "max_digits")?;
                let dp = extract_u32(s, "decimal_places")?;
                Some(FieldType::Decimal { max_digits: md, decimal_places: dp })
            }
            _ if s.starts_with("ForeignKey(") => {
                let to = extract_str(s, "to")?.to_string();
                Some(FieldType::ForeignKey { to, on_delete: OnDelete::Cascade, db_column: None })
            }
            _ if s.starts_with("OneToOneField(") => {
                let to = extract_str(s, "to")?.to_string();
                Some(FieldType::OneToOne { to, on_delete: OnDelete::Cascade })
            }
            _ if s.starts_with("ManyToManyField(") => {
                let to = extract_str(s, "to")?.to_string();
                Some(FieldType::ManyToMany { to, through: None, db_table: None })
            }
            _ => None,
        }
    }
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
