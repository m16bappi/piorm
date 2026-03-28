use sea_query::{Alias, Asterisk, Expr, ExprTrait, SimpleExpr};

/// An aggregate expression attached to a queryset via `annotate()` or `aggregate()`.
#[derive(Debug, Clone)]
pub enum AggregateExpr {
    Count { field: Option<String>, distinct: bool },
    Sum(String),
    Avg(String),
    Max(String),
    Min(String),
}

impl AggregateExpr {
    /// Convert to a sea-query `SimpleExpr` for use in SELECT / HAVING.
    pub fn to_sea_expr(&self) -> SimpleExpr {
        match self {
            Self::Count { field: None, .. } => {
                Expr::cust("COUNT(*)")
            }
            Self::Count { field: Some(f), distinct: true } => {
                Expr::cust(format!("COUNT(DISTINCT \"{f}\")"))
            }
            Self::Count { field: Some(f), distinct: false } => {
                Expr::col(Alias::new(f)).count()
            }
            Self::Sum(f) => Expr::col(Alias::new(f)).sum(),
            Self::Avg(f) => Expr::col(Alias::new(f)).avg(),
            Self::Max(f) => Expr::col(Alias::new(f)).max(),
            Self::Min(f) => Expr::col(Alias::new(f)).min(),
        }
    }

    /// SQL fragment for use in HAVING (repeats the expression — alias-safe for all backends).
    pub fn to_having_expr(&self) -> SimpleExpr {
        self.to_sea_expr()
    }
}

/// Wraps all columns in `Expr::col(Alias::new(...))` so sea-query can use them.
pub fn col(name: &str) -> SimpleExpr {
    Expr::col(Alias::new(name))
}

/// Build a SELECT * from an alias used in subquery wrappers.
pub fn all_from(alias: &str) -> SimpleExpr {
    Expr::col((Alias::new(alias), Asterisk))
}
