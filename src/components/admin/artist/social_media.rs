//! `SocialMedia` service management module.

use leptos::prelude::*;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::social_media::SocialMedia;
use crate::routes::links::{LinksResult, get_links};

/// Edit the social media services for an artist.
#[component]
pub fn SocialMediaServiceEdit(
    /// Artist slug to fetch social media services for.
    artist_slug: String,
) -> impl IntoView {
    let active_social_media_services = RwSignal::new(LinksResult::default().social_media_services);
    let other_social_media_services = RwSignal::new(vec![]);
    let social_media_service_resource = Resource::new(move || artist_slug.clone(), get_links);
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match social_media_service_resource.await {
                        Ok(services) => {
                            active_social_media_services.set(services.social_media_services);
                            let filtered_social_media_services = SocialMedia::iterator()
                                .filter(|platform| {
                                    !active_social_media_services
                                        .get()
                                        .iter()
                                        .any(|service| service.platform == **platform)
                                })
                                .collect::<Vec<_>>();
                            other_social_media_services.set(filtered_social_media_services);
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch social media services. Error: {e:?}");
                        }
                    }
                })} <div class="flex flex-wrap gap-4 justify-between">
                    <For
                        each=move || active_social_media_services.get()
                        key=|social_media_service| (social_media_service.platform.clone())
                        let(social_media_service)
                    >
                        <div class="shadow-sm card card-dash bg-accent text-base-content">
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
                                    <legend class="fieldset-legend">Social Media Link</legend>
                                    <input
                                        type="text"
                                        class="w-full input"
                                        placeholder="Social Media Link"
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
                        <div class="shadow-sm card card-dash bg-secondary text-base-content">
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
                                    <legend class="fieldset-legend">Social Media Link</legend>
                                    <input
                                        type="text"
                                        class="w-full input"
                                        placeholder="Social Media Link"
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
