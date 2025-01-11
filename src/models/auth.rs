use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub permissions: HashSet<String>,
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
            permissions,
        }
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

        pub async fn get_from_username(name: String, pool: &PgPool) -> Option<Self> {
            Self::get_from_username_with_passhash(name, pool)
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
        pub password: String,
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
                    permissions: sql_user_perms.map_or_else(HashSet::<String>::new, |user_perms| {
                        user_perms
                            .into_iter()
                            .map(|x| x.token)
                            .collect::<HashSet<String>>()
                    }),
                },
                UserPasshash(self.password),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_default() {
        let user = User::default();
        assert_eq!(user.id, -1);
        assert_eq!(user.username, "Guest");
        assert!(user.permissions.is_empty());
    }

    #[test]
    fn test_user_is_authenticated() {
        let user = User::default();
        assert!(!user.is_authenticated());

        let user = User {
            id: 1,
            username: "test".into(),
            permissions: HashSet::new(),
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
            permissions: HashSet::new(),
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
            permissions: HashSet::new(),
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
                permissions: HashSet::new(),
            };
            assert!(user.is_authenticated());
        }
    }
}
