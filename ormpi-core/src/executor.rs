use sea_query::Values;

use crate::pool::OrmpiPool;
use crate::value::OrmpiValue;

/// Execute a raw SQL statement that doesn't return rows (DDL / DML).
pub async fn execute_sql(pool: &OrmpiPool, sql: &str) -> crate::Result<u64> {
    pool.execute(sql, Values(vec![])).await
}

/// Execute a raw SELECT with optional positional params and return all rows as `OrmpiRow`.
pub async fn fetch_all_raw(
    pool: &OrmpiPool,
    sql: &str,
    params: &[OrmpiValue],
) -> crate::Result<Vec<crate::row::OrmpiRow>> {
    let values = Values(params.iter().map(|v| v.to_sea_value()).collect());
    pool.fetch_all(sql, values).await
}
