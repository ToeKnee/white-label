use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::PgPool;
use std::collections::HashSet;

use super::traits::Validate;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: String,
    pub description: Option<String>,
    pub permissions: HashSet<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// Explicitly is not Serialize/Deserialize!
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserPasshash(pub String);

impl Default for User {
    fn default() -> Self {
        let permissions = HashSet::new();

        Self {
            id: -1,
            username: "Guest".into(),
            first_name: None,
            last_name: None,
            email: "hello@example.com".into(),
            description: None,
            permissions,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

impl Validate for User {
    #[cfg(feature = "ssr")]
    async fn validate(&self, pool: &PgPool) -> anyhow::Result<()> {
        if self.username.is_empty() {
            return Err(anyhow::anyhow!("Username is required."));
        }
        if self.username.len() > 255 {
            return Err(anyhow::anyhow!(
                "Username must be less than 255 characters.".to_string()
            ));
        }
        // Check that the username is unique
        if let Ok(user) = Self::get_by_username(pool, self.username.clone()).await {
            if user.id != self.id {
                return Err(anyhow::anyhow!(
                    "Username or Email already taken.".to_string()
                ));
            }
        }

        if self.email.len() > 255 {
            return Err(anyhow::anyhow!(
                "Email must be less than 255 characters.".to_string()
            ));
        }
        if !self.email.contains('@') {
            return Err(anyhow::anyhow!("Email must be valid.".to_string()));
        }
        // Check that the email is unique
        if let Ok(user) = Self::get_by_email(pool, self.email.clone()).await {
            if user.id != self.id {
                return Err(anyhow::anyhow!(
                    "Username or Email already taken.".to_string()
                ));
            }
        }

        Ok(())
    }
}

impl User {
    pub const fn is_authenticated(&self) -> bool {
        self.id != -1
    }

    pub const fn is_active(&self) -> bool {
        self.id != -1
    }

    pub const fn is_anonymous(&self) -> bool {
        self.id == -1
    }

    /// Get user by username
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `username` - The username of the user
    ///
    /// # Returns
    /// The user
    ///
    /// # Errors
    /// If the user cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_username(pool: &PgPool, username: String) -> anyhow::Result<Self> {
        Self::get_from_username(username, pool)
            .await
            .map_or_else(|| Err(anyhow::anyhow!("User not found.")), Ok)
    }

    /// Get user by email
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `email` - The email of the user
    ///
    /// # Returns
    /// The user
    ///
    /// # Errors
    /// If the user cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_email(pool: &PgPool, email: String) -> anyhow::Result<Self> {
        Self::get_from_email(email, pool)
            .await
            .map_or_else(|| Err(anyhow::anyhow!("User not found.")), Ok)
    }

    /// Update an user
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The updated user
    ///
    /// # Errors
    /// If the user cannot be updated, return an error
    ///
    /// # Panics
    /// If the user cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn update(mut self, pool: &PgPool) -> anyhow::Result<Self> {
        self.validate(pool).await?;
        leptos::logging::log!("Updating user: {:?}", self);
        self.updated_at = chrono::Utc::now();
        sqlx::query(
            "UPDATE users SET username = $1, email = $2, description = $3, first_name = $4, last_name = $5, updated_at = $6 WHERE id = $7",
        )
        .bind(self.username.clone())
        .bind(self.email.clone())
        .bind(self.description.clone())
        .bind(self.first_name.clone())
        .bind(self.last_name.clone())
        .bind(self.updated_at)
        .bind(self.id)
        .execute(pool)
        .await?;

        Ok(self)
    }
}

#[cfg(feature = "ssr")]
pub mod ssr {
    pub use super::{User, UserPasshash};
    pub use axum_session_auth::Authentication;
    use axum_session_sqlx::SessionPgPool;
    pub use sqlx::PgPool;
    pub use std::collections::HashSet;
    pub type AuthSession = axum_session_auth::AuthSession<User, i64, SessionPgPool, PgPool>;
    pub use async_trait::async_trait;

    impl User {
        pub async fn get_with_passhash(id: i64, pool: &PgPool) -> Option<(Self, UserPasshash)> {
            let sqluser = sqlx::query_as::<_, SqlUser>("SELECT * FROM users WHERE id = $1")
                .bind(id)
                .fetch_one(pool)
                .await
                .ok()?;

            //lets just get all the tokens the user can use, we will only use the full permissions if modifying them.
            let sql_user_perms = sqlx::query_as::<_, SqlPermissionTokens>(
                "SELECT token FROM user_permissions WHERE user_id = $1",
            )
            .bind(id)
            .fetch_all(pool)
            .await
            .ok()?;

            Some(sqluser.into_user(Some(sql_user_perms)))
        }

        pub async fn get(id: i64, pool: &PgPool) -> Option<Self> {
            Self::get_with_passhash(id, pool)
                .await
                .map(|(user, _)| user)
        }

        pub async fn get_from_username_with_passhash(
            name: String,
            pool: &PgPool,
        ) -> Option<(Self, UserPasshash)> {
            let sqluser = sqlx::query_as::<_, SqlUser>("SELECT * FROM users WHERE username = $1")
                .bind(name)
                .fetch_one(pool)
                .await
                .ok()?;

            // Lets just get all the tokens the user can use, we will only use the full permissions if modifying them.
            let sql_user_perms = sqlx::query_as::<_, SqlPermissionTokens>(
                "SELECT token FROM user_permissions WHERE user_id = $1;",
            )
            .bind(sqluser.id)
            .fetch_all(pool)
            .await
            .ok()?;

            Some(sqluser.into_user(Some(sql_user_perms)))
        }

        pub async fn get_from_username(name: String, pool: &PgPool) -> Option<Self> {
            Self::get_from_username_with_passhash(name, pool)
                .await
                .map(|(user, _)| user)
        }

        pub async fn get_from_email_with_passhash(
            name: String,
            pool: &PgPool,
        ) -> Option<(Self, UserPasshash)> {
            let sqluser = sqlx::query_as::<_, SqlUser>("SELECT * FROM users WHERE email = $1")
                .bind(name)
                .fetch_one(pool)
                .await
                .ok()?;

            //lets just get all the tokens the user can use, we will only use the full permissions if modifying them.
            let sql_user_perms = sqlx::query_as::<_, SqlPermissionTokens>(
                "SELECT token FROM user_permissions WHERE user_id = $1;",
            )
            .bind(sqluser.id)
            .fetch_all(pool)
            .await
            .ok()?;

            Some(sqluser.into_user(Some(sql_user_perms)))
        }

        pub async fn get_from_email(name: String, pool: &PgPool) -> Option<Self> {
            Self::get_from_email_with_passhash(name, pool)
                .await
                .map(|(user, _)| user)
        }
    }

    #[derive(sqlx::FromRow, Clone)]
    pub struct SqlPermissionTokens {
        pub token: String,
    }

    #[async_trait]
    impl Authentication<Self, i64, PgPool> for User {
        async fn load_user(userid: i64, pool: Option<&PgPool>) -> Result<Self, anyhow::Error> {
            let pool = pool.unwrap();

            Self::get(userid, pool)
                .await
                .ok_or_else(|| anyhow::anyhow!("Cannot get user"))
        }

        fn is_authenticated(&self) -> bool {
            self.id == -1
        }

        fn is_active(&self) -> bool {
            self.id != -1
        }

        fn is_anonymous(&self) -> bool {
            self.id == -1
        }
    }

    #[derive(sqlx::FromRow, Clone, Debug)]
    pub struct SqlUser {
        pub id: i64,
        pub username: String,
        pub first_name: Option<String>,
        pub last_name: Option<String>,
        pub email: String,
        pub description: Option<String>,
        pub password: String,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }

    impl SqlUser {
        pub fn into_user(
            self,
            sql_user_perms: Option<Vec<SqlPermissionTokens>>,
        ) -> (User, UserPasshash) {
            (
                User {
                    id: self.id,
                    username: self.username,
                    first_name: self.first_name,
                    last_name: self.last_name,
                    email: self.email,
                    description: self.description,
                    permissions: sql_user_perms.map_or_else(HashSet::<String>::new, |user_perms| {
                        user_perms
                            .into_iter()
                            .map(|x| x.token)
                            .collect::<HashSet<String>>()
                    }),
                    created_at: self.created_at,
                    updated_at: self.updated_at,
                },
                UserPasshash(self.password),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::models::test_helpers::create_test_user;

    use super::*;

    #[test]
    fn test_user_default() {
        let user = User::default();
        assert_eq!(user.id, -1);
        assert_eq!(user.username, "Guest");
        assert!(user.permissions.is_empty());
    }

    #[sqlx::test]
    fn test_user_validate(pool: PgPool) {
        let user = User::default();
        assert!(user.validate(&pool).await.is_ok());
    }

    #[sqlx::test]
    fn test_user_validate_empty_username(pool: PgPool) {
        let user = User {
            username: String::new(),
            ..Default::default()
        };
        let result = user.validate(&pool).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Username is required.".to_string()
        );
    }

    #[sqlx::test]
    fn test_user_validate_username_less_than_255_characters(pool: PgPool) {
        let user = User {
            username: "a".repeat(256),
            ..Default::default()
        };
        let result = user.validate(&pool).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Username must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    fn test_user_validate_username_is_unique(pool: PgPool) {
        let original_user = create_test_user(&pool, 1).await.unwrap();
        let user = User {
            username: original_user.username,
            ..Default::default()
        };
        let result = user.validate(&pool).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Username or Email already taken.".to_string()
        );
    }

    #[sqlx::test]
    fn test_user_validate_email_less_than_255_characters(pool: PgPool) {
        let user = User {
            email: "a".repeat(256),
            ..Default::default()
        };
        let result = user.validate(&pool).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Email must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    fn test_user_validate_email_has_at_symbol(pool: PgPool) {
        let user = User {
            email: "invalid-email".to_string(),
            ..Default::default()
        };
        let result = user.validate(&pool).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Email must be valid.".to_string()
        );
    }

    #[sqlx::test]
    fn test_user_validate_email_is_unique(pool: PgPool) {
        let original_user = create_test_user(&pool, 1).await.unwrap();
        let user = User {
            email: original_user.email,
            ..Default::default()
        };
        let result = user.validate(&pool).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Username or Email already taken.".to_string()
        );
    }

    #[test]
    fn test_user_is_authenticated() {
        let user = User::default();
        assert!(!user.is_authenticated());

        let user = User {
            id: 1,
            username: "test".into(),
            first_name: None,
            last_name: None,
            email: "test@example.com".into(),
            description: None,
            permissions: HashSet::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(user.is_authenticated());
    }

    #[test]
    fn test_user_is_active() {
        let user = User::default();
        assert!(!user.is_active());

        let user = User {
            id: 1,
            username: "test".into(),
            first_name: None,
            last_name: None,
            email: "test@example.com".into(),
            description: None,
            permissions: HashSet::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(user.is_active());
    }

    #[test]
    fn test_user_is_anonymous() {
        let user = User::default();
        assert!(user.is_anonymous());

        let user = User {
            id: 1,
            username: "test".into(),
            first_name: None,
            last_name: None,
            email: "test@example.com".into(),
            description: None,
            permissions: HashSet::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(!user.is_anonymous());
    }

    #[cfg(feature = "ssr")]
    mod ssr_tests {
        use super::*;
        use axum_session_auth::Authentication;
        use bcrypt::verify;
        use sqlx::PgPool;
        use std::collections::HashSet;

        use crate::models::test_helpers::create_test_user;

        #[sqlx::test]
        async fn test_get_by_username(pool: PgPool) {
            let (test_user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);
            let user = User::get_by_username(&pool, "test-1".into()).await.unwrap();
            assert_eq!(user, test_user);

            let result = User::get_by_username(&pool, "test-2".into()).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "User not found.".to_string()
            );
        }

        #[sqlx::test]
        async fn test_get_by_email(pool: PgPool) {
            let (test_user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);
            let user = User::get_by_email(&pool, "test-1@example.com".into())
                .await
                .unwrap();
            assert_eq!(user, test_user);

            let result = User::get_by_email(&pool, "test-2@example.com".into()).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "User not found.".to_string()
            );
        }

        #[sqlx::test]
        async fn test_update(pool: PgPool) {
            let test_user = create_test_user(&pool, 1).await.unwrap();
            let mut user = User::get(1, &pool).await.unwrap();
            user.username = "test-2".into();
            user.email = "test-2@example.com".to_string();
            user.first_name = Some("Test".into());
            user.last_name = Some("User".into());
            user.description = Some("A description".into());
            let updated_user = user.update(&pool).await.unwrap();
            assert_eq!(updated_user.id, test_user.id);
            assert_eq!(updated_user.username, "test-2");
            assert_eq!(updated_user.email, "test-2@example.com");
            assert_eq!(updated_user.first_name, Some("Test".into()));
            assert_eq!(updated_user.last_name, Some("User".into()));
            assert_eq!(updated_user.description, Some("A description".into()));
        }

        #[sqlx::test]
        async fn test_get_with_passhash(pool: PgPool) {
            let test_user = create_test_user(&pool, 1).await.unwrap();
            let (user, UserPasshash(passhash)) = User::get_with_passhash(1, &pool).await.unwrap();
            assert_eq!(user.id, test_user.id);
            assert_eq!(user.username, test_user.username);
            assert_eq!(user.permissions, HashSet::new());
            assert!(verify("password", &passhash).unwrap());
        }

        #[sqlx::test]
        async fn test_get(pool: PgPool) {
            let test_user = create_test_user(&pool, 1).await.unwrap();
            let user = User::get(1, &pool).await.unwrap();
            assert_eq!(user.id, test_user.id);
            assert_eq!(user.username, test_user.username);
            assert_eq!(user.permissions, HashSet::new());
        }

        #[sqlx::test]
        async fn test_get_from_username_with_passhash(pool: PgPool) {
            let test_user = create_test_user(&pool, 1).await.unwrap();
            let (user, UserPasshash(passhash)) =
                User::get_from_username_with_passhash("test-1".into(), &pool)
                    .await
                    .unwrap();
            assert_eq!(user.id, test_user.id);
            assert_eq!(user.username, test_user.username);
            assert_eq!(user.permissions, HashSet::new());
            assert!(verify("password", &passhash).unwrap());
        }

        #[sqlx::test]
        async fn test_get_from_username(pool: PgPool) {
            let test_user = create_test_user(&pool, 1).await.unwrap();
            let user = User::get_from_username("test-1".into(), &pool)
                .await
                .unwrap();
            assert_eq!(user.id, test_user.id);
            assert_eq!(user.username, test_user.username);
            assert_eq!(user.permissions, HashSet::new());
        }

        #[sqlx::test]
        async fn test_load_user(pool: PgPool) {
            let test_user = create_test_user(&pool, 1).await.unwrap();
            let user = User::load_user(1, Some(&pool)).await.unwrap();
            assert_eq!(user.id, test_user.id);
            assert_eq!(user.username, test_user.username);
            assert_eq!(user.permissions, HashSet::new());
        }

        #[sqlx::test]
        async fn test_is_authenticated() {
            let user = User::default();
            assert!(!user.is_authenticated());

            let user = User {
                id: 1,
                username: "test".into(),
                first_name: None,
                last_name: None,
                email: "test@example.com".into(),
                description: None,
                permissions: HashSet::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            assert!(user.is_authenticated());
        }
    }
}
