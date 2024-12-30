use leptos::prelude::ServerFnError;
use leptos::server;

use crate::models::artist::Artist;
use crate::models::record_label::RecordLabel;

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct LabelResult {
    pub label: RecordLabel,
}

#[server]
pub async fn get_record_label() -> Result<LabelResult, ServerFnError> {
    Ok(LabelResult {
        label: RecordLabel::first().await.map_err(|x| {
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
pub async fn get_label_artists(
    record_label: RecordLabel,
) -> Result<LabelArtistResult, ServerFnError> {
    let artists = record_label.artists().await.map_err(|x| {
        let err = format!("Error while getting artists: {x:?}");
        tracing::error!("{err}");
        ServerFnError::new("Could not retrieve artists, try again later")
    })?;
    Ok(LabelArtistResult { artists })
}
