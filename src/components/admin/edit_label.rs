//! Edit Record Label Page

use leptos::prelude::*;
use leptos_meta::Title;
use reactive_stores::Store;

use crate::components::utils::{
    error::ErrorPage, error::ServerErrors, loading::Loading, permissions::permission_or_redirect,
    success::Success,
};
use crate::models::record_label::RecordLabel;
use crate::routes::record_label::LabelResult;
use crate::routes::record_label::UpdateRecordLabel;
use crate::store::{GlobalState, GlobalStateStoreFields};

/// Renders the edit record label page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
pub fn EditLabel() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let update_record_label = ServerAction::<UpdateRecordLabel>::new();
    let value = Signal::derive(move || {
        update_record_label
            .value()
            .get()
            .unwrap_or_else(|| Ok(LabelResult::default()))
    });
    let (success, set_success) = signal(false);

    let var_name = view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    view! {
                        <Title text=format!("Edit {}", store.record_label().get().name) />
                        <h1>"Edit "{store.record_label().get().name}" Details"</h1>
                        <ActionForm action=update_record_label>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(label_result) => {
                                            let record_label = label_result.label;
                                            if record_label.id > 0 {
                                                let store_record_label = store.record_label();
                                                *store_record_label.write() = record_label;
                                                set_success.set(true);
                                            } else {
                                                set_success.set(false);
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            set_success.set(false);
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                {move || {
                                    view! {
                                        <Success
                                            message=format!(
                                                "{} Updated!",
                                                store.record_label().get().name,
                                            )
                                            show=success.get()
                                        />
                                    }
                                }}
                                <input
                                    type="text"
                                    class="hidden"
                                    placeholder=""
                                    name="id"
                                    value=move || store.record_label().get().id
                                /> <div class="divider">Public</div>
                                <label class="flex gap-2 items-center input">
                                    <input
                                        type="text"
                                        class="grow"
                                        placeholder="Label name"
                                        name="name"
                                        value=move || store.record_label().get().name
                                    />
                                </label>
                                {move || {
                                    view! {
                                        <DescriptionFields record_label=store
                                            .record_label()
                                            .get() />
                                    }
                                }} <div class="divider">Private</div> <ISRCDescription />
                                <fieldset class="fieldset">
                                    <legend class="fieldset-legend">ISRC Code prefix</legend>
                                    <input
                                        type="text"
                                        class="w-full input"
                                        placeholder="ISRC prefix"
                                        name="isrc_base"
                                        value=move || store.record_label().get().isrc_base
                                    />
                                    <p class="flex justify-between label">
                                        <span>"Country Code and First Registrant Code only"</span>
                                        <span>
                                            "Example " {move || store.record_label().get().isrc_base}
                                            " 25 00001"
                                        </span>
                                    </p>
                                </fieldset> <button class="btn btn-primary">Update</button>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    };
    var_name
}

/// Managed description so we can preview markdown
#[component]
pub fn DescriptionFields(
    /// The record label
    record_label: RecordLabel,
) -> impl IntoView {
    let (description, set_description) = signal(record_label.description);
    let (markdown_description, set_markdown_description) = signal(String::new());
    Effect::new(move || {
        set_markdown_description.set(
            markdown::to_html_with_options(&description.get(), &markdown::Options::gfm())
                .unwrap_or_default(),
        );
    });

    view! {
        <div class="flex gap-6">
            <label class="w-1/2 fieldset">
                <h2>Description</h2>
                <textarea
                    class="w-full textarea"
                    rows="15"
                    name="description"
                    placeholder="Description"
                    prop:value=move || description.get()
                    on:input:target=move |ev| {
                        set_description.set(ev.target().value());
                    }
                >
                    {description}
                </textarea>
                <p class="flex justify-between label">
                    <span></span>
                    <span>"Markdown supported"</span>
                </p>
            </label>
            <div class="w-1/2">
                <h2>Preview</h2>
                <div inner_html=markdown_description />
            </div>
        </div>
    }
}

/// ISRC Description component
#[component]
pub fn ISRCDescription() -> impl IntoView {
    view! {
        <div class="collapse collapse-arrow bg-base-200">
            <input type="checkbox" />
            <div class="text-xl font-medium collapse-title">What is an ISRC code?</div>
            <div class="collapse-content">
                <p>
                    "An ISRC (International Standard Recording Code) is a unique 12-character alphanumeric identifier assigned to a specific recording of music. It serves as a standardized way to identify and track sound recordings across different territories, particularly within the digital marketplace for music distribution."
                </p>

                <p>
                    "The ISRC system was created by IFPI (International Federation of the Phonographic Industry) in 1973 and is used worldwide for tracking the use of sound recordings. Each ISRC code represents a specific version or release of a particular track, which can be owned by different labels in different countries. It includes information about the recording company, title, and publisher, as well as details about when and where the recording was produced."
                </p>

                <p>"ISRC codes are used primarily to:"</p>

                <dl>
                    <dt>"Track Usage"</dt>
                    <dd>
                        "Ensure that royalties are paid correctly for plays of a song on radio, TV, streaming services, or other media platforms across different countries."
                    </dd>
                    <dt>"Legal Purposes"</dt>
                    <dd>
                        "Support legal disputes related to copyright infringement by providing clear evidence of ownership and usage rights for a particular recording."
                    </dd>
                    <dt>"Sales and Marketing"</dt>
                    <dd>
                        "Facilitate the sale, licensing, or promotion of a music recording in various markets without needing to re-register it each time."
                    </dd>
                    <dt>"Technical Identification"</dt>
                    <dd>
                        "Aid in data management and metadata handling within digital music distribution platforms and databases."
                    </dd>
                </dl>

                <p>"To obtain an ISRC code for a new recording:"</p>

                <ol>
                    <li>
                        "The record producer, publisher, or their authorized representative applies to a national registration body that is part of the IFPI system. This can be done before or after the release of the recording."
                    </li>
                    <li>
                        "The application process involves providing detailed information about the recording, such as its title, artist(s), and creation date. A fee may also be charged for this service."
                    </li>
                    <li>
                        "Once registered, an ISRC code is assigned to each distinct version of the recording that can be identified by these attributes. This includes different formats (e.g., CD, digital file) or releases in a multi-track release like an album."
                    </li>
                    <li>
                        "The ISRC code should be included on all physical and digital copies of the recording as well as promotional materials to help identify it correctly."
                    </li>
                    <li>
                        "Over time, if there are changes to the recording (such as new versions, edits, or remasters), these would need to be registered again with a new ISRC code if needed for legal or business purposes."
                    </li>
                </ol>

                <p>
                    "By using an ISRC code, music creators and distributors can streamline the process of managing rights and royalties across borders, making it easier to comply with international licensing agreements and regulations."
                </p>
            </div>
        </div>
    }
}
