use leptos::prelude::*;

/// Date field
///
/// This field is used to set the date fields.
/// datetime-local input is used to allow the user to select a date and time, but this can't have a time zone.
/// To work around this, we add the current time zone to the input value.
#[component]
pub fn DateField<'a>(title: String, field: &'a str, date: Option<chrono::DateTime<chrono::Utc>>) -> impl IntoView {
    let date = RwSignal::new(date);
    let name = RwSignal::new(format!("local_{field}"));
    view! {
        <input
            type="text"
            class="hidden"
            name=field.to_string()
            prop:value=move || { date.get().map(|x| { x.to_string() }).unwrap_or_default() }
        />
        <fieldset class="w-full max-w-xs fieldset">
            <label class="flex justify-between label" for=move || { name.get() }>
                <span>{title}</span>
                <span>"*Date & Time"</span>
            </label>
            {move || {
                view! {
                    <input
                        class="w-full max-w-xs input"
                        type="datetime-local"
                        id=move || { name.get() }
                        name=move || { name.get() }
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
                                <label
                                    class="flex justify-between label"
                                    for=move || { name.get() }
                                >
                                    <span></span>
                                    <span>
                                        "Timezone " {p.format("%z").to_string()} " ("
                                        {p.format("%Z").to_string()}")"
                                    </span>
                                </label>
                            }
                                .into_any()
                        },
                    )
            }}
        </fieldset>
    }
}
