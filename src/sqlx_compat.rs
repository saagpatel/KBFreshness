pub use sqlx_core::error::Error;
pub use sqlx_core::from_row::FromRow;
pub use sqlx_core::row::Row;
pub use sqlx_core::sql_str::AssertSqlSafe;
use sqlx_core::sql_str::SqlSafeStr;
pub use sqlx_postgres::PgPool;
use sqlx_postgres::{PgRow, Postgres};

pub mod migrate {
    pub use sqlx_core::migrate::Migrator;
}

pub mod postgres {
    pub use sqlx_postgres::{PgPoolOptions, PgRow};
}

pub fn query<'q>(
    sql: impl SqlSafeStr,
) -> sqlx_core::query::Query<'q, Postgres, <Postgres as sqlx_core::database::Database>::Arguments> {
    sqlx_core::query::query::<Postgres>(sql)
}

pub fn query_as<'q, O>(
    sql: impl SqlSafeStr,
) -> sqlx_core::query_as::QueryAs<
    'q,
    Postgres,
    O,
    <Postgres as sqlx_core::database::Database>::Arguments,
>
where
    O: for<'r> FromRow<'r, PgRow>,
{
    sqlx_core::query_as::query_as::<Postgres, O>(sql)
}
