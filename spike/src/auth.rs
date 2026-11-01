//! Server-only auth glue: `axum_session_auth` wired to the main app's real
//! `users` and `user_permissions` tables, following Dioxus' `07-fullstack/auth`
//! example (`Session`/`AuthLayer` type aliases referenced by the server
//! function macro attributes).

use async_trait::async_trait;
use axum_session_auth::Authentication;
use axum_session_sqlx::SessionPgPool;
use sqlx::PgPool;

use crate::models::User;

pub(crate) type Session = axum_session_auth::AuthSession<User, i64, SessionPgPool, PgPool>;
pub(crate) type AuthLayer = axum_session_auth::AuthSessionLayer<User, i64, SessionPgPool, PgPool>;

#[derive(sqlx::FromRow)]
struct SqlUserRow {
    id: i64,
    username: String,
    first_name: Option<String>,
    last_name: Option<String>,
    email: String,
    description: Option<String>,
    avatar: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SqlPermissionToken {
    token: String,
}

#[async_trait]
impl Authentication<User, i64, PgPool> for User {
    async fn load_user(userid: i64, pool: Option<&PgPool>) -> Result<User, anyhow::Error> {
        let Some(pool) = pool else {
            return Err(anyhow::anyhow!(
                "No database pool was provided to load_user"
            ));
        };

        let Some(row) = sqlx::query_as::<_, SqlUserRow>(
            "SELECT id, username, first_name, last_name, email, description, avatar \
             FROM users WHERE id = $1",
        )
        .bind(userid)
        .fetch_optional(pool)
        .await?
        else {
            return Err(anyhow::anyhow!("User {userid} no longer exists"));
        };

        let tokens = sqlx::query_as::<_, SqlPermissionToken>(
            "SELECT token FROM user_permissions WHERE user_id = $1",
        )
        .bind(userid)
        .fetch_all(pool)
        .await?;

        Ok(User {
            id: row.id,
            username: row.username,
            first_name: row.first_name,
            last_name: row.last_name,
            email: row.email,
            description: row.description,
            avatar: row.avatar,
            permissions: tokens.into_iter().map(|token| token.token).collect(),
        })
    }

    fn is_authenticated(&self) -> bool {
        self.id != -1
    }

    fn is_active(&self) -> bool {
        self.id != -1
    }

    fn is_anonymous(&self) -> bool {
        self.id == -1
    }
}
