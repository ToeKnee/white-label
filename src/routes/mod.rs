//! This module contains the API endpoints for the application. It maps between the web API request and the service layer.

pub mod artist;
pub mod artists;
#[allow(clippy::unused_async)]
pub mod auth;
pub mod files;
pub mod page;
pub mod record_label;
pub mod release;
pub mod track;
