//! Auth endpoints ported from `routes/auth.rs` (see the `api` module docs
//! for the wire protocol). Sessions move from `axum_session_auth` to the
//! `tower-sessions` record + `CurrentUser` extractor.

use axum::response::Response;
use tower_sessions::Session;

use crate::models::auth::User;
use crate::routes::auth::{ChangePassword, GetUser, Login, Logout, Register, UpdateUser};
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{
    CurrentUser, Db, login as record_login, logout as record_logout, remember_session,
};
use crate::services::user::{
    change_password_service, login_user, register_user_service, update_user_service,
};

const PATH_USER: &str = "/api/user";
const PATH_LOGIN: &str = "/api/login";
const PATH_REGISTER: &str = "/api/register";
const PATH_LOGOUT: &str = "/api/logout";
const PATH_UPDATE_PROFILE: &str = "/api/update_profile";
const PATH_CHANGE_PASSWORD: &str = "/api/change_password";

/// Get the current user from the session.
///
/// Mirrors `routes/auth.rs::get_user`: `null` when nobody is logged in.
pub async fn get_user(
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    decode_args::<GetUser>(PATH_USER, &body)?;

    json_response(PATH_USER, &current_user)
}

/// Login a user with the provided username and password.
pub async fn login(Db(pool): Db, session: Session, body: String) -> Result<Response, WireError> {
    let args = decode_args::<Login>(PATH_LOGIN, &body)?;

    // `login_user` carries the empty-field, unknown-user and wrong-password
    // checks (same messages and statuses as this handler always had).
    let user = login_user(&pool, args.username, args.password)
        .await
        .map_err(|error| WireError::from_app_error(PATH_LOGIN, &error))?;

    record_login(&session, user.id)
        .await
        .map_err(|error| WireError::internal(PATH_LOGIN, error))?;
    remember_session(&session, args.remember.as_deref());

    json_response(PATH_LOGIN, &user)
}

/// Register a new user, then log the new user in (same as the Leptos
/// `register` route, which calls `login_user` after registering).
pub async fn register(Db(pool): Db, session: Session, body: String) -> Result<Response, WireError> {
    let args = decode_args::<Register>(PATH_REGISTER, &body)?;

    let user = register_user_service(&pool, args.form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_REGISTER, &error))?;

    record_login(&session, user.id)
        .await
        .map_err(|error| WireError::internal(PATH_REGISTER, error))?;
    remember_session(&session, args.remember.as_deref());

    json_response(PATH_REGISTER, &user)
}

/// End the current session and answer with the default (logged-out) user.
pub async fn logout(session: Session, body: String) -> Result<Response, WireError> {
    decode_args::<Logout>(PATH_LOGOUT, &body)?;

    record_logout(&session)
        .await
        .map_err(|error| WireError::internal(PATH_LOGOUT, error))?;

    json_response(PATH_LOGOUT, &User::default())
}

/// Update the current user's profile.
///
/// Unlike the Leptos route there is no `reload_user` step: the session only
/// stores the user id, so the fresh row the service returns is enough.
pub async fn update_user(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<UpdateUser>(PATH_UPDATE_PROFILE, &body)?;

    let user = update_user_service(&pool, current_user.as_ref(), args.user_form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_UPDATE_PROFILE, &error))?;

    json_response(PATH_UPDATE_PROFILE, &user)
}

/// Change the current user's password.
pub async fn change_password(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<ChangePassword>(PATH_CHANGE_PASSWORD, &body)?;

    let user = change_password_service(&pool, current_user.as_ref(), args.password_form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_CHANGE_PASSWORD, &error))?;

    json_response(PATH_CHANGE_PASSWORD, &user)
}

#[cfg(test)]
mod tests {
    use axum::body::Bytes;
    use sqlx::PgPool;

    use super::{Login, PATH_LOGIN};
    use crate::forms::user::{ChangePasswordForm, RegisterUserForm, UpdateUserForm};
    use crate::models::auth::User;
    use crate::models::test_helpers::create_test_user;
    use crate::routes::auth::{ChangePassword, GetUser, Logout, Register, UpdateUser};
    use crate::server::api::test_support::{api_server, post_args};

    fn login_args(username: &str, password: &str) -> Login {
        Login {
            username: username.to_string(),
            password: password.to_string(),
            remember: None,
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_returns_the_user_and_opens_a_session(pool: PgPool) {
        let created = create_test_user(&pool, 1).await.expect("test user");
        let server = api_server(pool).await;

        let response = post_args(&server, PATH_LOGIN, &login_args("test-1", "password")).await;

        response.assert_status_ok();
        let logged_in: User = response.json();
        assert_eq!(logged_in.id, created.id);
        assert_eq!(logged_in.username, "test-1");

        // axum-test replays the session cookie, so `/api/user` sees the login.
        let me = post_args(&server, "/api/user", &GetUser {}).await;
        me.assert_status_ok();
        let me: Option<User> = me.json();
        assert_eq!(me.map(|user| user.id), Some(created.id));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_user_without_login_is_null(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(&server, "/api/user", &GetUser {}).await;

        response.assert_status_ok();
        assert_eq!(response.text(), "null");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_unknown_user_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(&server, PATH_LOGIN, &login_args("ghost", "whatever")).await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|User does not exist.");
        assert_eq!(response.header("serverfnerror"), PATH_LOGIN);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_wrong_password_is_unauthorized(pool: PgPool) {
        create_test_user(&pool, 1).await.expect("test user");
        let server = api_server(pool).await;

        let response = post_args(&server, PATH_LOGIN, &login_args("test-1", "nope")).await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|Password does not match.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_empty_fields_is_bad_request(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(&server, PATH_LOGIN, &login_args("test-1", "")).await;

        response.assert_status_bad_request();
        assert_eq!(
            response.text(),
            "ServerError|Username and password are required."
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn malformed_body_is_an_args_error(pool: PgPool) {
        let server = api_server(pool).await;

        let response = server
            .post(PATH_LOGIN)
            .content_type("application/x-www-form-urlencoded")
            .bytes(Bytes::from_static(b"nonsense=1"))
            .await;

        response.assert_status_bad_request();
        assert!(
            response.text().starts_with("Args|"),
            "expected an Args| wire error, got {:?}",
            response.text()
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn logout_ends_the_session(pool: PgPool) {
        create_test_user(&pool, 1).await.expect("test user");
        let server = api_server(pool).await;

        post_args(&server, PATH_LOGIN, &login_args("test-1", "password"))
            .await
            .assert_status_ok();

        let logged_out = post_args(&server, "/api/logout", &Logout {}).await;
        logged_out.assert_status_ok();
        // Parity with the Leptos route, which answers the default user.
        let logged_out_user: User = logged_out.json();
        assert_eq!(logged_out_user.id, -1);

        let me: Option<User> = post_args(&server, "/api/user", &GetUser {}).await.json();
        assert_eq!(me, None, "the flushed session should be anonymous");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn register_creates_logs_in_and_rejects_duplicates(pool: PgPool) {
        let server = api_server(pool).await;
        let form = RegisterUserForm {
            username: "newcomer".to_string(),
            email: "newcomer@example.com".to_string(),
            password: "secret123".to_string(),
            password_confirmation: "secret123".to_string(),
        };

        let response = post_args(
            &server,
            "/api/register",
            &Register {
                form: form.clone(),
                remember: None,
            },
        )
        .await;

        response.assert_status_ok();
        let registered: User = response.json();
        assert_eq!(registered.username, "newcomer");

        let me: Option<User> = post_args(&server, "/api/user", &GetUser {}).await.json();
        assert_eq!(
            me.map(|user| user.username),
            Some("newcomer".to_string()),
            "register should log the new user in"
        );

        let dupe = post_args(
            &server,
            "/api/register",
            &Register {
                form,
                remember: None,
            },
        )
        .await;
        dupe.assert_status_bad_request();
        assert!(dupe.text().starts_with("ServerError|"));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn update_profile_changes_the_account(pool: PgPool) {
        create_test_user(&pool, 1).await.expect("test user");
        let server = api_server(pool).await;

        post_args(&server, PATH_LOGIN, &login_args("test-1", "password"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            "/api/update_profile",
            &UpdateUser {
                user_form: UpdateUserForm {
                    original_username: "test-1".to_string(),
                    username: "test-1".to_string(),
                    email: "fresh@example.com".to_string(),
                    description: Some("hello".to_string()),
                    first_name: None,
                    last_name: None,
                },
            },
        )
        .await;

        response.assert_status_ok();
        let updated: User = response.json();
        assert_eq!(updated.email, "fresh@example.com");
        assert_eq!(updated.description.as_deref(), Some("hello"));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn update_profile_for_unknown_original_is_internal(pool: PgPool) {
        // Matches current Leptos behaviour: no session + an original_username
        // that doesn't resolve → the service's internal error. (Refining the
        // status/message is follow-up work.)
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            "/api/update_profile",
            &UpdateUser {
                user_form: UpdateUserForm {
                    original_username: "ghost".to_string(),
                    username: "ghost".to_string(),
                    email: "ghost@example.com".to_string(),
                    description: None,
                    first_name: None,
                    last_name: None,
                },
            },
        )
        .await;

        response.assert_status_internal_server_error();
        assert_eq!(response.text(), "ServerError|Error while getting user.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn change_password_wrong_current_is_unauthorized(pool: PgPool) {
        create_test_user(&pool, 1).await.expect("test user");
        let server = api_server(pool).await;

        post_args(&server, PATH_LOGIN, &login_args("test-1", "password"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            "/api/change_password",
            &ChangePassword {
                password_form: ChangePasswordForm {
                    password: "nope".to_string(),
                    new_password: "brandnew".to_string(),
                    new_password_confirmation: "brandnew".to_string(),
                },
            },
        )
        .await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|Password does not match.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn change_password_happy_path_lets_the_user_relogin(pool: PgPool) {
        create_test_user(&pool, 1).await.expect("test user");
        let server = api_server(pool).await;

        post_args(&server, PATH_LOGIN, &login_args("test-1", "password"))
            .await
            .assert_status_ok();

        let changed = post_args(
            &server,
            "/api/change_password",
            &ChangePassword {
                password_form: ChangePasswordForm {
                    password: "password".to_string(),
                    new_password: "brandnew".to_string(),
                    new_password_confirmation: "brandnew".to_string(),
                },
            },
        )
        .await;
        changed.assert_status_ok();

        // Mismatched confirmation is caught up front (400).
        let mismatch = post_args(
            &server,
            "/api/change_password",
            &ChangePassword {
                password_form: ChangePasswordForm {
                    password: "brandnew".to_string(),
                    new_password: "third".to_string(),
                    new_password_confirmation: "other".to_string(),
                },
            },
        )
        .await;
        mismatch.assert_status_bad_request();
        assert_eq!(mismatch.text(), "ServerError|Passwords did not match.");

        post_args(&server, PATH_LOGIN, &login_args("test-1", "brandnew"))
            .await
            .assert_status_ok();
    }
}
