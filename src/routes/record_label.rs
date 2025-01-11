use leptos::prelude::ServerFnError;
use leptos::server;

use crate::models::artist::Artist;
use crate::models::record_label::RecordLabel;
#[cfg(feature = "ssr")]
use crate::state::pool;

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct LabelResult {
    pub label: RecordLabel,
}

#[server]
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

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct LabelArtistResult {
    pub artists: Vec<Artist>,
}

#[server]
pub async fn get_label_artists(record_label_id: i64) -> Result<LabelArtistResult, ServerFnError> {
    let pool = pool()?;
    let record_label = RecordLabel::get_by_id(&pool, record_label_id)
        .await
        .map_err(|x| {
            let err = format!("Error while getting label: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve label, try again later")
        })?;
    let artists = record_label.artists(&pool).await.map_err(|x| {
        let err = format!("Error while getting artists: {x:?}");
        tracing::error!("{err}");
        ServerFnError::new("Could not retrieve artists, try again later")
    })?;
    Ok(LabelArtistResult { artists })
}

#[server]
pub async fn update_record_label(
    id: i64,
    name: String,
    description: String,
    isrc_base: String,
) -> Result<LabelResult, ServerFnError> {
    use crate::state::auth;

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
