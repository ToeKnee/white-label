use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::{error::ErrorPage, error::InlineError, loading::Loading};
use crate::routes::record_label::LabelResult;
use crate::routes::record_label::UpdateRecordLabel;
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::split_at_colon;

/// Renders the edit record label page.
#[component]
pub fn EditLabel() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (record_label, set_record_label) = signal(store.record_label().get());

    let user_context = expect_context::<UserContext>();
    let (user, _set_user) = signal(user_context.0.get().clone());

    let update_record_label = ServerAction::<UpdateRecordLabel>::new();
    let value = Signal::derive(move || {
        update_record_label
            .value()
            .get()
            .unwrap_or_else(|| Ok(LabelResult::default()))
    });
    let (saved, set_saved) = signal(false);
    let (server_errors, set_server_errors) = signal(Option::<ServerFnError>::None);

    // Managed description so we can preview markdown
    let (description, set_description) = signal(record_label.get().description.clone());
    let (markdown_description, set_markdown_description) = signal("".to_string());
    Effect::new(move || {
        set_markdown_description.set(markdown::to_html(&description.get().clone()));
    });

    Effect::new_isomorphic(move || {
        if user.get().is_active() && !user.get().permissions.contains("label_owner") {
            let navigate = use_navigate();
            navigate("/", Default::default());
        }
    });

    let var_name = view! {
        <h1>Edit Record Label Details</h1>

        "TODO: Fix prettier errors*"
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    view! {
                        <ActionForm action=update_record_label>
                            <div class="grid gap-6">
                                {move || {
                                    match server_errors.get() {
                                        Some(errors) => {
                                            view! {
                                                <InlineError message=split_at_colon(&errors.to_string())
                                                    .1 />
                                            }
                                                .into_any()
                                        }
                                        None => view! { "" }.into_any(),
                                    }
                                }}
                                {move || {
                                    match value.get() {
                                        Ok(label_result) => {
                                            let record_label = label_result.label;
                                            if record_label.id > 0 {
                                                let store_record_label = store.record_label();
                                                *store_record_label.write() = record_label.clone();
                                                set_record_label.set(record_label.clone());
                                                set_saved.set(true);
                                                set_server_errors.set(None);
                                            }
                                        }
                                        Err(error) => {
                                            set_saved.set(false);
                                            set_server_errors.set(Some(error));
                                        }
                                    }
                                    if saved.get() == true {
                                        let record_label = value.get().unwrap().label;
                                        let store_record_label = store.record_label();
                                        *store_record_label.write() = record_label.clone();
                                        set_record_label.set(record_label.clone());

                                        view! {
                                            <div role="alert" class="alert alert-success">
                                                <svg
                                                    xmlns="http://www.w3.org/2000/svg"
                                                    class="w-6 h-6 stroke-current shrink-0"
                                                    fill="none"
                                                    viewBox="0 0 24 24"
                                                >
                                                    <path
                                                        stroke-linecap="round"
                                                        stroke-linejoin="round"
                                                        stroke-width="2"
                                                        d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                                                    />
                                                </svg>
                                                <span>{record_label.name}" Updated!"</span>
                                            </div>
                                        }
                                            .into_any()
                                    } else {
                                        view! { "" }.into_any()
                                    }
                                }}
                                <input
                                    type="text"
                                    class="hidden"
                                    placeholder=""
                                    name="id"
                                    value=record_label.get().id
                                /> <div class="divider">Public</div>
                                <label class="flex gap-2 items-center input input-bordered">
                                    <input
                                        type="text"
                                        class="grow"
                                        placeholder="Label name"
                                        name="name"
                                        value=record_label.get().name
                                    />
                                </label> <div class="flex gap-6">
                                    <label class="w-1/2 form-control">
                                        <textarea
                                            class="textarea textarea-bordered"
                                            rows="15"
                                            name="description"
                                            placeholder="Description"
                                            prop:value=move || description.get().clone()
                                            on:input:target=move |ev| {
                                                set_description.set(ev.target().value())
                                            }
                                        >
                                            {record_label.get().description}
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
                                </div> <div class="divider">Private</div>
                                <div class="collapse collapse-arrow bg-base-200">
                                    <input type="checkbox" />
                                    <div class="text-xl font-medium collapse-title">
                                        What is an ISRC code?
                                    </div>
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
                                </div> <label class="flex gap-2 form-control">
                                    <input
                                        type="text"
                                        class="grow input input-bordered"
                                        placeholder="ISRC prefix"
                                        name="isrc_base"
                                        value=record_label.get().isrc_base
                                    />
                                    <div class="label">
                                        <span class="label-text-alt">
                                            "Country Code and First Registrant Code only"
                                        </span>
                                        <span class="label-text-alt">
                                            "Example " {record_label.get().isrc_base} " 25 00001"
                                        </span>
                                    </div>
                                </label> <button class="btn btn-primary">Update</button>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    };
    var_name
}
