use sqlx::PgPool;

use crate::models::artist::Artist;
#[cfg(feature = "ssr")]
use crate::models::auth::{ssr::SqlPermissionTokens, ssr::SqlUser, User};
use crate::models::record_label::RecordLabel;

/// Create test artist
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the artist
/// * `record_label` - The record label the artist is signed to (optional)
///
/// # Returns
/// The created artist
///
/// # Errors
/// If the artist cannot be created, return an error
///
/// # Panics
/// If the artist cannot be created, panic
/// If the record label is not found or cannot be created, panic
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
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the record label
///
/// # Returns
/// The created record label
///
/// # Errors
/// If the record label cannot be created, return an error
///
/// # Panics
/// If the record label cannot be created, panic
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
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the user
///
/// # Returns
/// The created user
///
/// # Errors
/// If the user cannot be created, return an error
///
/// # Panics
/// If the user cannot be created, panic
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

/// Create test user with provided permissions
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the user
/// * `permissions` - The permissions to assign to the user
///
/// # Returns
/// The created user with the provided permissions
///
/// # Errors
/// If the user cannot be created, return an error
/// If the permissions cannot be assigned to the user, return an error
///
/// # Panics
/// If the user cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_user_with_permissions(
    pool: &PgPool,
    id: usize,
    permissions: Vec<&str>,
) -> Result<User, sqlx::Error> {
    let user = create_test_user(pool, id).await.unwrap();

    let mut permission_tokens = vec![];
    for permission in permissions.clone() {
        let token = sqlx::query_as::<_, SqlPermissionTokens>(
            "INSERT INTO user_permissions (user_id, token) VALUES ($1, $2) RETURNING *",
        )
        .bind(user.id)
        .bind(permission)
        .fetch_one(pool)
        .await?;
        permission_tokens.push(token);
    }

    let (user, _) = user.into_user(Some(permission_tokens));
    Ok(user)
}
