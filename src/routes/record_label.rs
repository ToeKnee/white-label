//! Routes for handling record label data.
use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::models::{artist::Artist, page::Page, record_label::RecordLabel};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

/// A result containing a single `RecordLabel`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct LabelResult {
    /// The record label being returned.
    pub label: RecordLabel,
}

/// A result containing a list of artists associated with a `RecordLabel`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct LabelArtistResult {
    /// A vector of artists associated with the record label.
    pub artists: Vec<Artist>,
}

/// A result containing a list of pages associated with a `RecordLabel`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct LabelPageResult {
    /// A vector of pages associated with the record label.
    pub pages: Vec<Page>,
}

/// Get the first record label in the database.
///
/// # Returns:
/// A `LabelResult` containing the first record label.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue retrieving the record label.
#[server(GetRecordLabel, "/api", endpoint="record_label", output = Cbor)]
pub async fn get_record_label() -> Result<LabelResult, ServerFnError> {
    let pool = pool()?;

    Ok(LabelResult {
        label: RecordLabel::first(&pool).await.map_err(|x| {
            let err = format!("Error while getting labels: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve labels, try again later")
        })?,
    })
}

/// Get the `Artitst` objects associated with a specific record label.
///
/// # Arguments:
/// * `record_label_id`: The ID of the record label.
///
/// # Returns:
/// A `LabelArtistResult` containing a vector of artists associated with the specified record label.
///
/// # Errors:
/// Will return a `ServerFnError` if the record label cannot be found, or if there is an issue with the database connection.
#[server(GetLabelArtists, "/api", endpoint="record_label_artists", output = Cbor)]
pub async fn get_label_artists(
    /// The ID of the record label.
    record_label_id: i64,
) -> Result<LabelArtistResult, ServerFnError> {
    let auth = auth()?;
    let pool = pool()?;

    let current_user = auth.current_user.unwrap_or_default();
    let show_hidden = current_user.permissions.contains("label_owner");

    let record_label = RecordLabel::get_by_id(&pool, record_label_id)
        .await
        .map_err(|x| {
            let err = format!("Error while getting label: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve label, try again later")
        })?;
    let artists = record_label
        .artists(&pool, show_hidden)
        .await
        .map_err(|x| {
            let err = format!("Error while getting artists: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve artists, try again later")
        })?;
    Ok(LabelArtistResult { artists })
}

/// Get the pages associated with a specific record label.
///
/// # Arguments:
/// * `record_label_id`: The ID of the record label.
///
/// # Returns:
/// A `LabelPageResult` containing a vector of pages associated with the specified record label.
///
/// # Errors:
/// Will return a `ServerFnError` if the record label cannot be found, or if there is an issue with the database connection.
#[server(GetLabelPages, "/api", endpoint="record_label_pages", output = Cbor)]
pub async fn get_label_pages(
    /// The ID of the record label.
    record_label_id: i64,
) -> Result<LabelPageResult, ServerFnError> {
    let auth = auth()?;
    let pool = pool()?;

    let current_user = auth.current_user.unwrap_or_default();
    let show_hidden = current_user.permissions.contains("label_owner");

    let record_label = RecordLabel::get_by_id(&pool, record_label_id)
        .await
        .map_err(|x| {
            let err = format!("Error while getting label: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve label, try again later")
        })?;
    let pages = record_label.pages(&pool, show_hidden).await.map_err(|x| {
        let err = format!("Error while getting pages: {x:?}");
        tracing::error!("{err}");
        ServerFnError::new("Could not retrieve pages, try again later")
    })?;
    Ok(LabelPageResult { pages })
}

/// Update a record label with the provided details.
///
/// # Arguments:
/// * `id`: The ID of the record label to update.
/// * `name`: The new name for the record label.
/// * `description`: The new description for the record label.
/// * `isrc_base`: The new ISRC base for the record label.
///
/// # Returns:
/// A `LabelResult` containing the updated record label.
///
/// # Errors:
/// Will return a `ServerFnError` if the user is not authenticated, does not have permission to update the label, or if there is an issue with the database connection.
#[server(UpdateRecordLabel, "/api", endpoint="update_record_label", output = Cbor)]
pub async fn update_record_label(
    /// The ID of the record label to update.
    id: i64,
    /// The new name for the record label.
    name: String,
    /// The new description for the record label.
    description: String,
    /// The new ISRC base for the record label.
    isrc_base: String,
) -> Result<LabelResult, ServerFnError> {
    let auth = auth()?;
    let pool = pool()?;

    let current_user = auth
        .current_user
        .as_ref()
        .ok_or_else(|| ServerFnError::new("You must be logged in to update a label"))?;
    if !current_user.permissions.contains("label_owner") {
        return Err(ServerFnError::new(
            "You do not have permission to update a label",
        ));
    }

    let mut record_label = RecordLabel::get_by_id(&pool, id).await.map_err(|x| {
        let err = format!("Error while getting label: {x:?}");
        tracing::error!("{err}");
        ServerFnError::new("Could not retrieve label, try again later")
    })?;

    record_label.name = name;
    record_label.description = description;
    record_label.isrc_base = isrc_base;
    match record_label.clone().update(&pool).await {
        Ok(record_label) => Ok(LabelResult {
            label: record_label,
        }),
        Err(e) => {
            let err = format!("Error while updating label: {e}");
            tracing::error!("{err}");
            Err(ServerFnError::new(
                "Could not update label, try again later",
            ))
        }
    }
}
