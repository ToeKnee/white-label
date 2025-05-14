use leptos::prelude::*;

/// Managed description so we can preview markdown
#[component]
pub fn MarkdownField(title: String, field: String, markdown_text: String) -> impl IntoView {
    let (description, set_description) = signal(markdown_text);
    let (markdown_description, set_markdown_description) = signal(String::new());
    Effect::new(move || {
        set_markdown_description.set(markdown::to_html_with_options(&description.get(), &markdown::Options::gfm()).unwrap_or_default());
    });

    view! {
        <div class="flex gap-6">
            <div class="w-1/2">
                <h2>{title.clone()}</h2>
                <fieldset class="fieldset">
                    <textarea
                        class="w-full textarea"
                        rows="15"
                        id=field.clone()
                        name=field.clone()
                        placeholder=title
                        prop:value=move || description.get()
                        on:input:target=move |ev| {
                            set_description.set(ev.target().value());
                        }
                    >
                        {description}
                    </textarea>

                    <label class="flex justify-between label" for=field>
                        <span></span>
                        <span>"Markdown Supported"</span>
                    </label>
                </fieldset>
            </div>
            <div class="w-1/2">
                <h2>Preview</h2>
                <div class="w-full textarea" inner_html=markdown_description />
            </div>
        </div>
    }
}
