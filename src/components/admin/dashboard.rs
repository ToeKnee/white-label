//! The dashboard admin page.
use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{
    admin::dash::{
        artists::ArtistsTable, pages::PagesTable, record_label::RecordLabel,
        social_media_tips::SocialMediaTips, upcoming_release::UpcomingRelease,
    },
    utils::{error::ErrorPage, loading::Loading},
};

/// Renders the record label page.
#[component]
pub fn Dashboard() -> impl IntoView {
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    view! {
                        <Title text="Admin Dashboard" />
                        <div class="flex flex-row flex-wrap gap-6 justify-between">
                            <RecordLabel />
                            <ArtistsTable />
                            <PagesTable />
                            <SocialMediaTips />
                            <UpcomingRelease />

                            <div class="shadow-xl grow not-prose card bg-neutral text-neutral-content">
                                <div class="card-body">
                                    <h2 class="card-title">Mailing list</h2>
                                    <div class="shadow stats">
                                        <div class="stat">
                                            <div class="stat-figure text-primary">
                                                <svg
                                                    xmlns="http://www.w3.org/2000/svg"
                                                    fill="none"
                                                    viewBox="0 0 24 24"
                                                    class="inline-block w-8 h-8 stroke-current"
                                                >
                                                    <path
                                                        stroke-linecap="round"
                                                        stroke-linejoin="round"
                                                        stroke-width="2"
                                                        d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"
                                                    ></path>
                                                </svg>
                                            </div>
                                            <div class="stat-title text-neutral-content">Email:</div>
                                            <div class="stat-value text-primary">0</div>
                                            <div class="stat-desc">subscribers</div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
