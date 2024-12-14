use leptos::prelude::ServerFnError;
use leptos::server;
// use leptos_router::*;

use crate::models::label::Label;

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct LabelResult {
    pub label: Label,
}

#[server(GetLabelAction, "/api", "GetJson")]
pub async fn get_label() -> Result<LabelResult, ServerFnError> {
    Ok(LabelResult {
        label: Label::first().await.map_err(|x| {
            let err = format!("Error while getting labels: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve labels, try again later")
        })?,
    })
}
