//! This module is the entry point of the application, it will setup the database, the web server and the logger.
//! It will also serve the application.

use leptos::prelude::get_configuration;
use leptos::*;
use leptos_axum::{generate_route_list, LeptosRoutes};

use crate::app::{shell, WhiteLabelRoot};

/// # Panics
///
/// Will panic if anything is badly setup from database, or web server
pub async fn init_app(configuration_path: Option<&str>) {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(tracing::Level::INFO)
        .init();
    // Init the pool into static
    crate::database::init_db()
        .await
        .expect("problem during initialization of the database");

    // Get leptos configuration
    let conf = get_configuration(configuration_path).unwrap();
    let addr = conf.leptos_options.site_addr;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(|| view! { <WhiteLabelRoot /> });
    let leptos_options = conf.leptos_options;

    let app = axum::Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
