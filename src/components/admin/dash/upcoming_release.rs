use leptos::prelude::*;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::release::Release;
use crate::routes::release::get_next_scheduled_release;

#[derive(Clone, Debug, Default)]
struct Countdown {
    days: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
}
impl Countdown {
    fn view_days(&self) -> String {
        format!("--value:{}", self.days)
    }

    fn view_hours(&self) -> String {
        format!("--value:{}", self.hours)
    }

    fn view_minutes(&self) -> String {
        format!("--value:{}", self.minutes)
    }

    fn view_seconds(&self) -> String {
        format!("--value:{}", self.seconds)
    }
}

/// A component that displays a countdown for a new release.
#[component]
pub fn UpcomingRelease() -> impl IntoView {
    let release = RwSignal::new(Release::default());
    let release_resource = Resource::new(move || {}, move |()| get_next_scheduled_release(None));
    let count_down = RwSignal::new(Countdown::default());

    Effect::new(move || {
        let now = chrono::Utc::now();
        let release_date = release
            .get()
            .release_date
            .map_or_else(chrono::Utc::now, |date| date);
        let diff = release_date - now;
        count_down.set(Countdown {
            days: diff.num_days(),
            hours: diff.num_hours() % 24,
            minutes: diff.num_minutes() % 60,
            seconds: diff.num_seconds() % 60,
        });
    });
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match release_resource.await {
                        Ok(Some(upcoming_release)) => {
                            release.set(upcoming_release.release);
                        }
                        Ok(None) => {
                            return view! { "" }.into_any();
                        }
                        Err(_) => {}
                    }

                    view! {
                        <div class="shadow-xl grow not-prose card bg-neutral text-neutral-content bg-base-100">
                            <div class="card-body">
                                <h2 class="card-title">Upcoming Release {release.get().name}</h2>
                                <div class="grid grid-flow-col auto-cols-max gap-5 text-center justify-center-safe">
                                    <div class="flex flex-col">
                                        <span class="font-mono text-5xl countdown">
                                            <span style=count_down.get().view_days()></span>
                                        </span>
                                        days
                                    </div>
                                    <div class="flex flex-col">
                                        <span class="font-mono text-5xl countdown">
                                            <span style=count_down.get().view_hours()></span>
                                        </span>
                                        hours
                                    </div>
                                    <div class="flex flex-col">
                                        <span class="font-mono text-5xl countdown">
                                            <span style=count_down.get().view_minutes()></span>
                                        </span>
                                        min
                                    </div>
                                    <div class="flex flex-col">
                                        <span class="font-mono text-5xl countdown">
                                            <span style=count_down.get().view_seconds()></span>
                                        </span>
                                        sec
                                    </div>
                                </div>
                            </div>
                        </div>
                    }
                        .into_any()
                })}
            </ErrorBoundary>
        </Transition>
    }
}
