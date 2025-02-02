use leptos::prelude::*;

use crate::models::artist::Artist;

/// Managed description so we can preview markdown
#[component]
pub fn DescriptionFields(artist: Artist) -> impl IntoView {
    let (description, set_description) = signal(artist.description);
    let (markdown_description, set_markdown_description) = signal(String::new());
    Effect::new(move || {
        set_markdown_description.set(
            markdown::to_html_with_options(&description.get(), &markdown::Options::gfm())
                .unwrap_or_default(),
        );
    });

    view! {
        <div class="flex gap-6">
            <label class="w-1/2 form-control">
                <h2>Description</h2>
                <textarea
                    class="textarea textarea-bordered"
                    rows="15"
                    name="artist_form[description]"
                    placeholder="Description"
                    prop:value=move || description.get()
                    on:input:target=move |ev| {
                        set_description.set(ev.target().value());
                    }
                >
                    {description}
                </textarea>
                <div class="label">
                    <span class="label-text-alt"></span>
                    <span class="label-text-alt">Markdown</span>
                </div>
            </label>
            <div class="w-1/2">
                <h2>Preview</h2>
                <div inner_html=markdown_description />
            </div>
        </div>
    }
}

/// Published at field
///
/// This field is used to set the published_at date for the artist.
/// datetime-local input is used to allow the user to select a date and time, but this can't have a time zone.
/// To work around this, we add the current time zone to the input value.
#[component]
pub fn PublishedAtField(published_at: Option<chrono::DateTime<chrono::Utc>>) -> impl IntoView {
    let published_at = RwSignal::new(published_at);

    view! {
        <input
            type="text"
            class="hidden"
            name="artist_form[published_at]"
            prop:value=move || { published_at.get().map(|x| { x.to_string() }).unwrap_or_default() }
        />
        <label class="w-full max-w-xs form-control">
            <div class="label">
                <span class="label-text">"Publish at"</span>
                <span class="label-text-alt">"*Date & Time"</span>
            </div>
            {move || {
                view! {
                    <input
                        class="w-full max-w-xs input input-bordered"
                        type="datetime-local"
                        name="local_published_at"
                        value=published_at
                            .get()
                            .map(|x| { x.format("%Y-%m-%dT%H:%M").to_string() })
                            .unwrap_or_default()
                        on:input:target=move |ev| {
                            let published_at_with_tz = format!(
                                "{}:00 {}",
                                ev.target().value(),
                                chrono::Utc::now().format("%z"),
                            );
                            let value = chrono::DateTime::parse_from_str(
                                    &published_at_with_tz,
                                    "%Y-%m-%dT%H:%M:%S %z",
                                )
                                .map_or(None, |v| Some(v.into()));
                            published_at.set(value);
                        }
                    />
                }
            }}
            {move || {
                published_at
                    .get()
                    .map_or_else(
                        || {

                            view! { "" }
                                .into_any()
                        },
                        |p| {
                            view! {
                                <div class="label">
                                    <span class="label-text-alt"></span>
                                    <span class="label-text-alt">
                                        "Timezone " {p.format("%z").to_string()} " ("
                                        {p.format("%Z").to_string()}")"
                                    </span>
                                </div>
                            }
                                .into_any()
                        },
                    )
            }}
        </label>
    }
}
