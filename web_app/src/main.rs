//! Web App service which provides the frontend for Telegram Bot.

#![allow(clippy::empty_structs_with_brackets, clippy::same_name_method)] // Triggered by leptos
#![allow(
    clippy::missing_docs_in_private_items,
    clippy::panic,
    clippy::expect_used
)]

use std::rc::Rc;

use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use js_sys::Reflect;
use leptos::{
    html::{ElementDescriptor, Input, Textarea},
    *,
};
use leptos_router::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::SubmitEvent;

#[wasm_bindgen]
extern "C" {
    /// Telegram Web App object initialized by a [Telegram JS script](https://telegram.org/js/telegram-web-app.js).
    ///
    /// For all possible methods and fields see https://core.telegram.org/bots/webapps#initializing-mini-apps.
    type WebApp;

    /// Expand [`WebApp`] to the maximum available size.
    #[wasm_bindgen(method)]
    fn expand(this: &WebApp);

    /// Enable confirmation dialog when closing a [`WebApp`].
    #[wasm_bindgen(method)]
    fn enableClosingConfirmation(this: &WebApp);

    /// Send data to the bot backend and close a [`WebApp`].
    ///
    /// `data` must be no longer than 4096 bytes.
    #[wasm_bindgen(method, catch)]
    fn sendData(this: &WebApp, data: JsValue) -> Result<(), JsValue>;
}

/// Payload with user data to be encrypted and sent to the bot backend.
#[derive(Serialize, Deserialize)]
struct Payload {
    /// Resource name.
    resource_name: String,
    /// Login.
    login: String,
    /// Password.
    password: String,
    /// Any additional comments.
    comments: String,
}

/// Main component.
#[component]
fn App() -> impl IntoView {
    let window = web_sys::window().expect("No window found");
    let telegram =
        Reflect::get(&window, &JsValue::from_str("Telegram")).expect("No Telegram found in window");
    let web_app = Reflect::get(&telegram, &JsValue::from_str("WebApp"))
        .expect("No WebApp found in window.Telegram");

    // `WebApp` is not a class, so checked casts like `dyn_into` fail.
    let web_app = web_app.unchecked_into::<WebApp>();
    web_app.expand();

    let web_app = Rc::new(web_app);

    let (submission_result, set_submission_result) = create_signal(Ok(()));
    let (show_result, set_show_result) = create_signal(Ok(()));

    view! {
        <Router>
            <Routes>
                <Route path="/submit" view=move || view! {
                    <Submit web_app=Rc::clone(&web_app) set_result=set_submission_result/>
                }/>
                <Route path="/show" view=move || view! {
                    <ShowComponent set_result=set_show_result/>
                }/>
                <Route path="/*any" view=|| view! { <h1>"Not Found"</h1> }/>
            </Routes>
        </Router>
        <ErrorBoundary fallback=|errors| view! {
            <div class = "error">
                { move || {
                    errors.get()
                    .into_iter()
                    .map(|(_, e)| view! { <p>{e.to_string()}</p>})
                    .collect_view()
                }}
            </div>
        }>
            { submission_result }
            { show_result }
        </ErrorBoundary>
    }
}

/// Error during new password submission.
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
enum SubmissionError {
    /// Invalid input: {0}
    Validation(&'static str),
    /// Failed to encrypt data
    Encryption(#[from] telepass_crypto::Error),
    /// Failed to serialize data: {0}
    Serialization(String),
    /// Failed to send data to the bot backend: {0}
    Sending(String),
}

impl From<serde_json::Error> for SubmissionError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

/// Component with input forms and `Submit` button.
///
/// Clicking on the button will send encrypted info to the bot via `web_app` and close the app.
#[component]
fn Submit(
    web_app: Rc<WebApp>,
    set_result: WriteSignal<Result<(), SubmissionError>>,
) -> impl IntoView {
    web_app.enableClosingConfirmation();

    let (resource_name, _set_resource_name) = new_input_param::<Input>(String::new(), false);
    let (login, _set_login) = new_input_param::<Input>(String::new(), false);
    let (password, _set_password) = new_input_param::<Input>(String::new(), false);
    let (comments, _set_comments) = new_input_param::<Textarea>(String::new(), false);
    let master_password_element = create_node_ref::<Input>();

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default(); // Prevent page reload

        let resource_name = resource_name
            .element
            .get()
            .expect("No resource_name element")
            .value();

        let payload = Payload {
            resource_name: resource_name.clone(),
            login: login.element.get().expect("No login element").value(),
            password: password.element.get().expect("No password element").value(),
            comments: comments.element.get().expect("No comments element").value(),
        };
        let master_password = master_password_element()
            .expect("No master_password element")
            .value();

        set_result(|| -> Result<(), SubmissionError> {
            let resource_name = resource_name.trim();
            if resource_name.is_empty() {
                return Err(SubmissionError::Validation("Resource name cannot be empty"));
            }

            let encryption_output = telepass_crypto::encrypt(
                &serde_json::to_value(payload)?.to_string(),
                &master_password,
            )?;

            let new_record = telepass_data_model::NewRecord {
                resource_name: resource_name.to_owned(),
                encryption_output,
            };

            // Telegram JS code checks some additional properties of the data (e.g. length),
            // So it's easier to serialize it to JSON and send as a string rather than use
            // something like `serde_wasm_bindgen`.
            web_app
                .sendData(serde_json::to_value(new_record)?.to_string().into())
                .map_err(|err| SubmissionError::Sending(format!("{err:?}")))
        }());
    };

    view! {
        <RecordInput
            resource_name=resource_name
            login=login
            password=password
            comments=comments
            master_password_element=master_password_element
            submit_value="Submit"
            on_submit=on_submit
        />
    }
}

/// Error during password presentation.
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
enum ShowError {
    /// Failed to parse query
    ParamsParsing(#[from] ParamsError),
    /// Both payload and salt must be provided
    MissingParams,
    /// Failed to decode payload or salt
    Base64Decoding(#[from] base64::DecodeError),
    /// Wrong salt length
    WrongSaltLength,
    /// Failed to decrypt data
    Decryption(#[from] telepass_crypto::Error),
    /// Failed to deserialize data: {0}
    Deserialization(String),
}

impl From<serde_json::Error> for ShowError {
    fn from(e: serde_json::Error) -> Self {
        Self::Deserialization(e.to_string())
    }
}

#[derive(Params, Clone, PartialEq, Eq)]
struct ShowParams {
    payload: Option<String>,
    salt: Option<String>,
}

#[component]
fn ShowComponent(set_result: WriteSignal<Result<(), ShowError>>) -> impl IntoView {
    let payload_and_salt = use_query::<ShowParams>()
        .get_untracked()
        .map_err(ShowError::from)
        .and_then(|params| match (params.payload, params.salt) {
            (Some(payload), Some(salt)) => Ok((payload, salt)),
            _ => Err(ShowError::MissingParams),
        })
        .and_then(|(url_encoded_payload, url_encoded_salt)| {
            URL_SAFE
                .decode(url_encoded_payload)
                .and_then(|payload| {
                    URL_SAFE
                        .decode(url_encoded_salt)
                        .map(|salt| (payload, salt))
                })
                .map_err(Into::into)
        })
        .and_then(|(payload, salt)| {
            Ok((
                payload,
                salt.try_into().map_err(|_err| ShowError::WrongSaltLength)?,
            ))
        })
        .map_err(|err| {
            set_result(Err(err));
        })
        .ok();

    let (resource_name, set_resource_name) = new_input_param(String::new(), true);
    let (login, set_login) = new_input_param(String::new(), true);
    let (password, set_password) = new_input_param(String::new(), true);
    let (comments, set_comments) = new_input_param(String::new(), true);
    let master_password_element = create_node_ref::<Input>();

    let on_decrypt = move |event: SubmitEvent| {
        event.prevent_default(); // Prevent page reload

        let Some((payload, salt)) = payload_and_salt.clone() else {
            return;
        };

        let master_password = master_password_element()
            .expect("No master_password element")
            .value();

        let payload = telepass_crypto::decrypt(
            telepass_crypto::EncryptionOutput {
                encrypted_payload: payload,
                salt,
            },
            &master_password,
        )
        .map_err(ShowError::from)
        .and_then(|record| serde_json::from_str::<Payload>(&record).map_err(Into::into));

        let payload = match payload {
            Ok(payload) => payload,
            Err(err) => {
                set_result(Err(err));
                return;
            }
        };
        set_resource_name.value.set(payload.resource_name);
        set_login.value.set(payload.login);
        set_password.value.set(payload.password);
        set_comments.value.set(payload.comments);
    };

    view! {
        <RecordInput
            resource_name=resource_name
            login=login
            password=password
            comments=comments
            master_password_element=master_password_element
            submit_value="Decrypt"
            on_submit=on_decrypt
        />
    }
}

struct FieldComponentParamRead<T: ElementDescriptor + 'static> {
    value: ReadSignal<String>,
    readonly: ReadSignal<bool>,
    element: NodeRef<T>,
}

impl<T: ElementDescriptor + 'static> Copy for FieldComponentParamRead<T> {}

impl<T: ElementDescriptor + 'static> Clone for FieldComponentParamRead<T> {
    fn clone(&self) -> Self {
        *self
    }
}

struct FieldComponentParamWrite {
    value: WriteSignal<String>,
    _readonly: WriteSignal<bool>,
}

fn new_input_param<T: ElementDescriptor + 'static>(
    value: String,
    readonly: bool,
) -> (FieldComponentParamRead<T>, FieldComponentParamWrite) {
    let (value, set_value) = create_signal(value);
    let (readonly, set_readonly) = create_signal(readonly);
    (
        FieldComponentParamRead {
            value,
            readonly,
            element: create_node_ref::<T>(),
        },
        FieldComponentParamWrite {
            value: set_value,
            _readonly: set_readonly,
        },
    )
}

#[component]
fn RecordInput<F: Fn(SubmitEvent) + 'static>(
    resource_name: FieldComponentParamRead<Input>,
    login: FieldComponentParamRead<Input>,
    password: FieldComponentParamRead<Input>,
    comments: FieldComponentParamRead<Textarea>,
    master_password_element: NodeRef<Input>,
    submit_value: &'static str,
    on_submit: F,
) -> impl IntoView {
    let resource_name_element = resource_name.element;
    let login_element = login.element;
    let password_element = password.element;
    let comments_element = comments.element;

    view! {
        <form on:submit=on_submit>
            <label for="resource_name">Resource name</label>
            <input type="text" id="resource_name" prop:value=resource_name.value readonly=resource_name.readonly node_ref=resource_name_element/>

            <label for="login">Login</label>
            <input type="text" id="login" prop:value=login.value readonly=login.readonly node_ref=login_element/>

            <label for="password">Password</label>
            <input type="password" id="password" prop:value=password.value readonly=password.readonly node_ref=password_element/>

            <details>
                <summary>Comments</summary>
                <textarea id="comments" prop:value=comments.value readonly=comments.readonly node_ref=comments_element/>
            </details>

            <label for="master-password">Master Password</label>
            <input type="password" id="master-password" node_ref=master_password_element/>

            <input type="submit" value=submit_value/>
        </form>
    }
}

fn main() {
    leptos::mount_to_body(App)
}
