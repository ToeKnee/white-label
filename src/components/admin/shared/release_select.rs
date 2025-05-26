use leptos::prelude::*;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::release::Release;
use crate::routes::artists::get_releases_for_artists;

fn toggle_release_id(release_ids: RwSignal<Vec<i64>>, release_id: i64) {
    let mut ids = release_ids.get();
    if ids.contains(&release_id) {
        ids.retain(|&id| id != release_id);
    } else {
        ids.push(release_id);
    }
    release_ids.set(ids);
}

fn ids_to_str(ids: &[i64]) -> String {
    ids.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join(",")
}

#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn ReleaseSelect(artist_ids: Vec<i64>, primary_release: Release, initial_release_ids: Vec<i64>) -> impl IntoView {
    let (releases, set_releases) = signal(vec![]);
    let releases_resource = Resource::new(move || ids_to_str(&artist_ids.clone()), get_releases_for_artists);
    let release_ids = RwSignal::new(initial_release_ids);

    Effect::new_isomorphic(move || {
        tracing::info!("ReleaseSelect: release_ids: {:?}", release_ids.get());
    });
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if releases.get().is_empty() {
                        if let Ok(release_list) = releases_resource.await {
                            set_releases.set(release_list.releases);
                        }
                    }
                    view! {
                        <div class="flex flex-col gap-4">
                            <h2>"Releases"</h2>
                            <legend class="label">
                                <span class="label-text">"Main Release"</span>
                            </legend>
                            <select class="select" name="form[primary_release_id]">
                                {move || {
                                    let release_rows = releases
                                        .get()
                                        .into_iter()
                                        .map(|release| {
                                            let checked = release.id == primary_release.id;
                                            view! {
                                                <option class="option" value=release.id selected=checked>
                                                    {release.name}
                                                </option>
                                            }
                                        })
                                        .collect::<Vec<_>>();
                                    view! { {release_rows} }
                                }}
                            </select>
                            {move || {
                                let release_rows = releases
                                    .get()
                                    .into_iter()
                                    .map(|release| {
                                        let checked = release_ids.get().contains(&release.id);
                                        view! {
                                            <label class="flex flex-row gap-4 label bg-base-100 border-base-300 rounded-box">

                                                <input
                                                    class="checkbox"
                                                    type="checkbox"
                                                    checked=checked
                                                    value=release.id
                                                    on:input:target=move |_ev| {
                                                        toggle_release_id(release_ids, release.id);
                                                    }
                                                />
                                                <div class="avatar not-prose">
                                                    <div class="w-8 rounded-full">
                                                        <img
                                                            src=release.primary_image_url()
                                                            alt=release.name.clone()
                                                        />
                                                    </div>
                                                </div>
                                                {release.name}
                                            </label>
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                if release_rows.is_empty() {
                                    view! { <p>"No releases found…"</p> }.into_any()
                                } else {
                                    view! {
                                        <fieldset class="flex flex-row flex-wrap gap-6 justify-center p-4 fieldset">
                                            <legend class="label">
                                                <span class="label-text">"All Releases"</span>
                                            </legend>
                                            {release_rows}
                                        </fieldset>
                                    }
                                        .into_any()
                                }
                            }}
                        </div>
                    }
                })}
                <input
                    type="text"
                    class="hidden"
                    name="form[release_ids]"
                    value=move || ids_to_str(&release_ids.get())
                />
            </ErrorBoundary>
        </Transition>
    }
}
