//! Server-only: shared database state for server functions.
//!
//! The pool is injected through `axum::Extension<Db>` (declared as an
//! extractor argument in each server function's macro attribute). A newtype
//! avoids the orphan-rule problem of implementing `FromRef` for `PgPool`
//! itself, which `dioxus::fullstack`'s `State<T>` extractor would need.

/// Wrapper around the Postgres pool so axum can hand it to server functions.
#[derive(Clone)]
pub struct Db(pub sqlx::PgPool);
