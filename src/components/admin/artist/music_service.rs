//! Music service management module.

use leptos::prelude::*;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::music_service::Platform;
use crate::routes::links::{LinksResult, get_links};

/// Edit the music services for an artist.
#[component]
pub fn MusicServiceEdit(
    /// Artist slug to fetch music services for.
    artist_slug: String,
) -> impl IntoView {
    let active_music_services = RwSignal::new(LinksResult::default().music_services);
    let other_music_services = RwSignal::new(vec![]);
    let music_service_resource = Resource::new(move || artist_slug.clone(), get_links);
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match music_service_resource.await {
                        Ok(services) => {
                            active_music_services.set(services.music_services);
                            let filtered_music_services = Platform::iterator()
                                .filter(|platform| {
                                    !active_music_services
                                        .get()
                                        .iter()
                                        .any(|service| service.platform == **platform)
                                })
                                .collect::<Vec<_>>();
                            other_music_services.set(filtered_music_services);
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch music services. Error: {e:?}");
                        }
                    }
                })} <div class="flex flex-wrap gap-4 justify-between">
                    <For
                        each=move || active_music_services.get()
                        key=|music_service| (music_service.platform.clone())
                        let(music_service)
                    >
                        <div class="shadow-sm card card-dash bg-accent text-base-content">
                            <div class="card-body">
                                <figure>
                                    <img
                                        class="w-auto h-12"
                                        src=format!(
                                            "/images/music_services/{}.svg",
                                            music_service.platform.to_string(),
                                        )
                                        alt=music_service.platform.clone().to_string()
                                    />
                                </figure>

                                <fieldset class="fieldset">
                                    <legend class="fieldset-legend">Music Service Link</legend>
                                    <input
                                        type="text"
                                        class="w-full input"
                                        placeholder="Music Service Link"
                                        value=music_service.url
                                    />
                                </fieldset>
                            </div>
                        </div>
                    </For>
                    <For
                        each=move || other_music_services.get()
                        key=|platform| (platform.to_string())
                        let(platform)
                    >
                        <div class="shadow-sm card card-dash bg-secondary text-base-content">
                            <div class="card-body">
                                <figure>
                                    <img
                                        class="w-auto h-12"
                                        src=format!(
                                            "/images/music_services/{}.svg",
                                            platform.clone().to_string(),
                                        )
                                        alt=platform.clone().to_string()
                                    />
                                </figure>

                                <fieldset class="fieldset">
                                    <legend class="fieldset-legend">Music Service Link</legend>
                                    <input
                                        type="text"
                                        class="w-full input"
                                        placeholder="Music Service Link"
                                    />
                                </fieldset>
                            </div>
                        </div>
                    </For>
                </div>
            </ErrorBoundary>
        </Transition>
    }
}
