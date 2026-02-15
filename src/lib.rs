extern crate self as sqlx;

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod health;
pub mod jobs;
pub mod security;
pub mod sources;
pub mod sqlx_compat;

pub use sqlx_compat::{migrate, postgres, query, query_as, Error, FromRow, PgPool, Row};
