use sqlx::PgPool;

use crate::models::artist::Artist;
#[cfg(feature = "ssr")]
use crate::models::auth::ssr::SqlUser;
use crate::models::record_label::RecordLabel;

/// Create test artist
#[cfg(feature = "ssr")]
pub async fn create_test_artist(
    pool: &PgPool,
    id: usize,
    record_label: Option<RecordLabel>,
) -> Result<Artist, sqlx::Error> {
    let record_label = match record_label {
        Some(label) => label,
        None => create_test_record_label(pool, id).await.unwrap(),
    };
    let artist = sqlx::query_as::<_, Artist>(
    "INSERT INTO artists (name, slug, description, label_id) VALUES ($1, $2, $3, $4) RETURNING *",
)
    .bind(format!("Test Artist {id}"))
    .bind(format!("test-artist-{id}"))
    .bind(format!("A artist for testing purposes with the id of {id}"))
    .bind(record_label.id)
    .fetch_one(pool)
    .await?;

    Ok(artist)
}

/// Create test record label
#[cfg(feature = "ssr")]
pub async fn create_test_record_label(
    pool: &PgPool,
    id: usize,
) -> Result<RecordLabel, sqlx::Error> {
    let label = sqlx::query_as::<_, RecordLabel>(
    "INSERT INTO labels (name, slug, description, isrc_base) VALUES ($1, $2, $3, $4) RETURNING *",
)
    .bind(format!("Test User {id}"))
    .bind(format!("test-user-{id}"))
    .bind(format!("A user for testing purposes with the id of {id}"))
    .bind(format!("UK AAA {id}"))
    .fetch_one(pool)
    .await?;

    Ok(label)
}

/// Create test user
#[cfg(feature = "ssr")]
pub async fn create_test_user(pool: &PgPool, id: usize) -> Result<SqlUser, sqlx::Error> {
    let user = sqlx::query_as::<_, SqlUser>(
        "INSERT INTO users (username, password) VALUES ($1, $2) RETURNING *",
    )
    .bind(format!("test-{id}"))
    .bind("$2b$12$bvHwxi3jnJC6/nzyFmKKBOZPHo/kn5KHPKxTeG0wiGOUlKuYYjZH.") // This is a valid bcrypt hash for the word "password"
    .fetch_one(pool)
    .await?;

    Ok(user)
}
