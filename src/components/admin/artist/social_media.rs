//! `SocialMedia` service management module.

use convert_case::{Case, Casing};
use leptos::prelude::*;

use crate::models::social_media::{SocialMedia, SocialMediaService};

/// Edit the social media services for an artist.
#[component]
pub fn SocialMediaServiceEdit(
    /// The active social media services for the artist.
    active_social_media_services: RwSignal<Vec<SocialMediaService>>,
) -> impl IntoView {
    let other_social_media_services = RwSignal::new(vec![]);

    Effect::new(move || {
        // Get all platforms that are not in the active music services
        let filtered_music_services = SocialMedia::iterator()
            .filter(|platform| {
                !active_social_media_services
                    .get()
                    .iter()
                    .any(|service| service.platform == **platform)
            })
            .collect::<Vec<_>>();
        other_social_media_services.set(filtered_music_services);
    });

    view! {
        <div class="flex flex-wrap gap-4 justify-between">
            <For
                each=move || active_social_media_services.get()
                key=|social_media_service| (social_media_service.platform.clone())
                let(social_media_service)
            >
                <div class="shadow-sm card bg-accent text-base-content">
                    <div class="card-body">
                        <figure>
                            <img
                                class="w-32 h-auto"
                                src=format!(
                                    "/images/social_media_services/{}.svg",
                                    social_media_service.platform.to_string(),
                                )
                                alt=social_media_service.platform.clone().to_string()
                            />
                        </figure>

                        <fieldset class="fieldset">
                            <legend class="fieldset-legend text-accent-content">
                                {social_media_service.platform.to_string().to_case(Case::Title)}
                            </legend>
                            <input
                                name=move || {
                                    format!(
                                        "form[{}]",
                                        social_media_service
                                            .platform
                                            .clone()
                                            .to_string()
                                            .to_case(Case::Snake),
                                    )
                                }
                                type="text"
                                class="w-full input"
                                placeholder="https://"
                                value=social_media_service.url
                            />
                        </fieldset>
                    </div>
                </div>
            </For>
            <For
                each=move || other_social_media_services.get()
                key=|platform| (platform.to_string())
                let(platform)
            >
                <div class="shadow-sm card bg-secondary text-base-content">
                    <div class="card-body">
                        <figure>
                            <img
                                class="w-32 h-auto"
                                src=format!(
                                    "/images/social_media_services/{}.svg",
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
