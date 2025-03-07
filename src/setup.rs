//! This file is the entry point for the server. It sets up the server and runs it.
//!
//! It also sets up the database connection and the session store.
//! It also sets up the routes and the handlers for the server.

use axum::{
    Router,
    body::Body as AxumBody,
    extract::{Path, State},
    http::Request,
    response::{IntoResponse, Response},
    routing::get,
};
use axum_session::{SessionConfig, SessionLayer, SessionStore};
use axum_session_auth::{AuthConfig, AuthSessionLayer};
use axum_session_sqlx::SessionPgPool;
use leptos::{config::get_configuration, prelude::provide_context};
use leptos_axum::{LeptosRoutes, generate_route_list, handle_server_fns_with_context};
use sqlx::PgPool;
use tower_http::services::ServeDir;

use crate::app::{WhiteLabel, shell};
use crate::database::create_pool;
use crate::models::auth::{User, ssr::AuthSession};
use crate::state::AppState;

async fn server_fn_handler(
    State(app_state): State<AppState>,
    auth_session: AuthSession,
    path: Path<String>,
    request: Request<AxumBody>,
) -> impl IntoResponse {
    tracing::debug!("server_fn_handler {:?}", path);
    // tracing::debug!("{:?}", request);
    handle_server_fns_with_context(
        move || {
            provide_context(auth_session.clone());
            provide_context(app_state.pool.clone());
        },
        request,
    )
    .await
}

async fn leptos_routes_handler(
    auth_session: AuthSession,
    state: State<AppState>,
    req: Request<AxumBody>,
) -> Response {
    let State(app_state) = state.clone();
    let handler = leptos_axum::render_route_with_context(
        app_state.routes.clone(),
        move || {
            provide_context(auth_session.clone());
            provide_context(app_state.pool.clone());
        },
        move || shell(app_state.leptos_options.clone()),
    );
    tracing::debug!("leptos_routes_handler {:?}", req);
    handler(state, req).await.into_response()
}

/// Initialise the application.
///
/// # Panics
///
/// This function will panic if it can't initialise the logger.
#[allow(clippy::cognitive_complexity)]
pub async fn init_app() {
    // Initialise the logger
    tracing_subscriber::fmt::init();

    // Set up the database
    let pool = create_pool().await;

    // Auth section
    let session_config = SessionConfig::default().with_table_name("axum_sessions");
    let auth_config = AuthConfig::<i64>::default();
    let session_store = match SessionStore::<SessionPgPool>::new(
        Some(SessionPgPool::from(pool.clone())),
        session_config,
    )
    .await
    {
        Ok(store) => store,
        Err(e) => {
            tracing::error!("Couldn't initialise session store: {:?}", e);
            return;
        }
    };

    // Setting this to None means we'll be using cargo-leptos and its env vars
    let conf = match get_configuration(None) {
        Ok(conf) => conf,
        Err(e) => {
            tracing::error!("Couldn't get configuration: {:?}", e);
            return;
        }
    };
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(WhiteLabel);

    let app_state = AppState {
        leptos_options,
        pool: pool.clone(),
        routes: routes.clone(),
    };

    let Ok(upload_path) = std::env::var("UPLOAD_PATH") else {
        tracing::error!("UPLOAD_PATH not set.");
        return;
    };

    // build our application with a route
    let app = Router::new()
        .route(
            "/api/*fn_name",
            get(server_fn_handler).post(server_fn_handler),
        )
        .nest_service("/uploads", ServeDir::new(upload_path))
        .leptos_routes_with_handler(routes, get(leptos_routes_handler))
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .layer(
            AuthSessionLayer::<User, i64, SessionPgPool, PgPool>::new(Some(pool.clone()))
                .with_config(auth_config),
        )
        .layer(SessionLayer::new(session_store))
        .with_state(app_state);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            tracing::info!("Listening on http://{}", &addr);
            listener
        }
        Err(e) => {
            tracing::error!("Couldn't bind address: {:?}", e);
            return;
        }
    };
    let serve = axum::serve(listener, app.into_make_service()).await;
    match serve {
        Ok(()) => tracing::info!("Server stopped."),
        Err(e) => tracing::error!("Server Error: {:?}", e),
    }
}
