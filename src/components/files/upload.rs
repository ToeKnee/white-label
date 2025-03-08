use futures::StreamExt;
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;
use web_sys::{FormData, HtmlFormElement, SubmitEvent};

use crate::config::upload::UploadConfiguration;
use crate::routes::files::upload::{file_progress, upload_file};

/// This component uses server functions to upload a file, while streaming updates on the upload
/// progress.
#[component]
pub fn FileUploadWithProgress(config: UploadConfiguration, slug: String) -> impl IntoView {
    let (filename, set_filename) = signal(None);
    let (max, set_max) = signal(None);
    let (current, set_current) = signal(None);
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let target = if let Some(target) = ev.target() {
            target.unchecked_into::<HtmlFormElement>()
        } else {
            tracing::error!("Couldn't get form target.");
            return;
        };
        let form_data = match FormData::new_with_form(&target) {
            Ok(form_data) => form_data,
            Err(e) => {
                tracing::error!("Couldn't get form data: {:?}", e);
                return;
            }
        };
        let file = form_data
            .get("file_to_upload")
            .unchecked_into::<web_sys::File>();
        let filename = file.name();
        let size = file.size();
        tracing::debug!("Size {}", size);
        set_filename.set(Some(filename.clone()));
        set_max.set(Some(size));
        set_current.set(None);

        spawn_local(async move {
            let mut progress = match file_progress(filename).await {
                Ok(progress) => progress.into_inner(),
                Err(e) => {
                    tracing::error!("Couldn't get progress stream: {e}");
                    return;
                }
            };
            while let Some(Ok(len)) = progress.next().await {
                // The TextStream from the server function will be a series of `usize` values
                // however, the response itself may pack those chunks into a smaller number of
                // chunks, each with more text in it
                // so we've padded them with newspace, and will split them out here
                // each value is the latest total, so we'll just take the last one.
                let len = match len.split('\n').filter(|n| !n.is_empty()).last() {
                    Some(len) => match len.parse::<usize>() {
                        Ok(len) => len,
                        Err(e) => {
                            tracing::error!("Couldn't parse length: {e}");
                            continue;
                        }
                    },
                    None => continue,
                };
                set_current.set(Some(len));
            }
        });
        spawn_local(async move {
            match upload_file(form_data.into()).await {
                Ok(()) => tracing::debug!("File uploaded."), // TODO: Refresh the page or user or something
                Err(e) => tracing::error!("Couldn't upload file: {e}"),
            }
        });
    };

    view! {
        <form on:submit=on_submit>
            <div class="grid gap-6">
                <div class="flex flex-auto gap-6">
                    <input type="hidden" name="type" value=config.to_string() />
                    <input type="hidden" name="slug" value=slug />
                    <input
                        type="file"
                        class="w-full file-input file-input-bordered file-input-primary"
                        name="file_to_upload"
                    />
                </div>
                <div class="flex flex-auto gap-6">
                    <button class="flex-1 btn btn-primary">Upload</button>
                </div>
            </div>
        </form>
        {move || filename.get().map(|filename| view! { <p>"Uploading "{filename}</p> })}
        {move || {
            max.get()
                .map(|max| {
                    view! {
                        <progress
                            class="w-56 progress progress-primary"
                            max=max
                            value=move || current.get().unwrap_or_default()
                        ></progress>
                    }
                })
        }}
    }
}
