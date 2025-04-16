use leptos::prelude::*;

/// Managed description so we can preview markdown
#[component]
pub fn MarkdownField(title: String, field: String, markdown_text: String) -> impl IntoView {
    let (description, set_description) = signal(markdown_text);
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
                <h2>{title.clone()}</h2>
                <textarea
                    class="textarea textarea-bordered"
                    rows="15"
                    name=field
                    placeholder=title
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
                <div class="textarea textarea-bordered" inner_html=markdown_description />
            </div>
        </div>
    }
}

/// Date field
///
/// This field is used to set the date fields.
/// datetime-local input is used to allow the user to select a date and time, but this can't have a time zone.
/// To work around this, we add the current time zone to the input value.
#[component]
pub fn DateField(
    title: String,
    field: String,
    date: Option<chrono::DateTime<chrono::Utc>>,
) -> impl IntoView {
    let date = RwSignal::new(date);

    view! {
        <input
            type="text"
            class="hidden"
            name=field.clone()
            prop:value=move || { date.get().map(|x| { x.to_string() }).unwrap_or_default() }
        />
        <label class="w-full max-w-xs form-control">
            <div class="label">
                <span class="label-text">{title}</span>
                <span class="label-text-alt">"*Date & Time"</span>
            </div>
            {move || {
                let name = format!("local_{field}");
                view! {
                    <input
                        class="w-full max-w-xs input input-bordered"
                        type="datetime-local"
                        name=name
                        value=date
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
                            date.set(value);
                        }
                    />
                }
            }}
            {move || {
                date.get()
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
