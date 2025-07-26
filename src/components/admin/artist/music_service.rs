//! Music service management module.

use convert_case::{Case, Casing};
use leptos::prelude::*;

use crate::models::music_service::{MusicService, Platform};

/// Edit the music services for an artist.
#[component]
pub fn MusicServiceEdit(
    /// Active music services.
    active_music_services: RwSignal<Vec<MusicService>>,
) -> impl IntoView {
    let other_music_services = RwSignal::new(vec![]);

    Effect::new(move || {
        // Get all platforms that are not in the active music services
        let filtered_music_services = Platform::iterator()
            .filter(|platform| {
                !active_music_services
                    .get()
                    .iter()
                    .any(|service| service.platform == **platform)
            })
            .collect::<Vec<_>>();
        other_music_services.set(filtered_music_services);
    });

    view! {
        <div class="flex flex-wrap gap-4 justify-between">
            <For
                each=move || active_music_services.get()
                key=|music_service| (music_service.platform.clone())
                let(music_service)
            >
                <div class="shadow-sm card bg-accent">
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
                            <legend class="fieldset-legend text-accent-content">
                                {music_service.platform.to_string().to_case(Case::Title)}
                            </legend>
                            <input
                                name=move || {
                                    format!(
                                        "form[{}]",
                                        music_service
                                            .platform
                                            .clone()
                                            .to_string()
                                            .to_case(Case::Snake),
                                    )
                                }
                                type="text"
                                class="w-full input"
                                placeholder="https://"
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
                <div class="shadow-sm card bg-secondary">
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
                            <legend class="fieldset-legend text-secondary-content">
                                {platform.clone().to_string().to_case(Case::Title)}
                            </legend>
                            <input
                                name=move || {
                                    format!(
                                        "form[{}]",
                                        platform.clone().to_string().to_case(Case::Snake),
                                    )
                                }
                                type="text"
                                class="w-full input"
                                placeholder="https://"
                            />
                        </fieldset>
                    </div>
                </div>
            </For>
        </div>
    }
}
