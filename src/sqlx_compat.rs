pub use sqlx_core::error::Error;
pub use sqlx_core::from_row::FromRow;
pub use sqlx_core::row::Row;
pub use sqlx_postgres::PgPool;
use sqlx_postgres::{PgRow, Postgres};

pub mod migrate {
    pub use sqlx_core::migrate::Migrator;
}

pub mod postgres {
    pub use sqlx_postgres::{PgPoolOptions, PgRow};
}

pub fn query(
    sql: &str,
) -> sqlx_core::query::Query<'_, Postgres, <Postgres as sqlx_core::database::Database>::Arguments<'_>>
{
    sqlx_core::query::query::<Postgres>(sql)
}

pub fn query_as<'q, O>(
    sql: &'q str,
) -> sqlx_core::query_as::QueryAs<
    'q,
    Postgres,
    O,
    <Postgres as sqlx_core::database::Database>::Arguments<'q>,
>
where
    O: for<'r> FromRow<'r, PgRow>,
{
    sqlx_core::query_as::query_as::<Postgres, O>(sql)
}
