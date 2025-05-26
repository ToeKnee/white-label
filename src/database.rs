use anyhow::Context;
use sqlx::postgres::PgPoolOptions;

/// Create a database connection pool.
///
/// # Returns
///
/// A `PgPool` connection pool.
///
/// # Errors
///
/// If the `.env` file is not found, return an error.
/// If the `DATABASE_URL` environment variable is not set, return an error.
/// If the migrations fail, return an error.
///
/// # Panics
///
/// If the database connection fails, the program will panic.
pub async fn create_pool() -> sqlx::PgPool {
    // Set up database connection
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL environment variable must be set.");

    let database_url = match database_url {
        Ok(database_url) => database_url,
        Err(e) => {
            handle_error(e);
        }
    };

    let pool = match PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url.as_str())
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            handle_error(format!(
                "Could not connect to database_url {database_url}: {e}"
            ));
        }
    };

    match sqlx::migrate!().run(&pool).await {
        Ok(()) => (),
        Err(e) => {
            handle_error(e);
        }
    }

    pool
}

fn handle_error(e: impl std::fmt::Display) -> ! {
    tracing::error!("{e}");
    std::process::exit(1)
}
