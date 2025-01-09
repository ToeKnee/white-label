use anyhow::Context;
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::sync::OnceLock;

static DB: OnceLock<sqlx::PgPool> = OnceLock::new();

async fn create_pool() -> sqlx::PgPool {
    // Load environment variables form env file.
    let _ = dotenv().context(".env file not found");

    // Set up database connection
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL environment variable must be set.");

    let database_url = match database_url {
        Ok(database_url) => database_url,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url.as_str())
        .await
        .expect("Could not connect to database_url {database_url}.");

    let _ = sqlx::migrate!()
        .run(&pool)
        .await
        .context("Migrations failed.");

    pool
}

pub async fn init_db() -> Result<(), sqlx::Pool<sqlx::Postgres>> {
    DB.set(create_pool().await)
}

pub fn get_db<'a>() -> &'a sqlx::PgPool {
    DB.get().expect("Database unitialized.")
}
