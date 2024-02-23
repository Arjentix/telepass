//! Web App service which provides the frontend for Telegram Bot.

#![allow(clippy::empty_structs_with_brackets, clippy::same_name_method)] // Triggered by leptos
#![allow(
    clippy::missing_docs_in_private_items,
    clippy::panic,
    clippy::expect_used
)]

use std::rc::Rc;

use js_sys::Reflect;
use leptos::{
    html::{Input, Textarea},
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

    /// Expand [`WepApp`] to the maximum available size.
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
    /// Login.
    login: String,
    /// Password.
    password: String,
    /// Any additional comments.
    comments: String,
}

/// Error during new password submission.
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
enum SubmissionError {
    /// Invalid input: {0}
    Validation(&'static str),
    /// Failed to encrypt password
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

    view! {
        <Router>
            <Routes>
                <Route path="/submit" clone:web_app view = move || view! {
                    <Submit web_app=Rc::clone(&web_app) set_result=set_submission_result/>
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
        </ErrorBoundary>
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

    let resource_name_element = create_node_ref::<Input>();
    let login_element = create_node_ref::<Input>();
    let password_element = create_node_ref::<Input>();
    let comments_element = create_node_ref::<Textarea>();
    let master_password_element = create_node_ref::<Input>();

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default(); // Prevent page reload

        let resource_name = resource_name_element()
            .expect("No resource_name element")
            .value();

        let payload = Payload {
            login: login_element().expect("No login element").value(),
            password: password_element().expect("No password element").value(),
            comments: comments_element().expect("No comments element").value(),
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

            let new_password = telepass_data_model::NewRecord {
                resource_name: resource_name.to_owned(),
                encryption_output,
            };

            // Telegram JS code checks some additional properties of the data (e.g. length),
            // So it's easier to serialize it to JSON and send as a string rather than use
            // something like `serde_wasm_bindgen`.
            web_app
                .sendData(serde_json::to_value(new_password)?.to_string().into())
                .map_err(|err| SubmissionError::Sending(format!("{err:?}")))
        }());
    };

    view! {
        <form on:submit=on_submit>
            <label for="resource_name">Resource name</label>
            <input type="text" id="resource_name" node_ref=resource_name_element/>

            <label for="login">Login</label>
            <input type="text" id="login" node_ref=login_element/>

            <label for="password">Password</label>
            <input type="password" id="password" node_ref=password_element/>

            <details>
                <summary>Comments</summary>
                <textarea id="comments" node_ref=comments_element/>
            </details>

            <label for="master-password">Master Password</label>
            <input type="password" id="master-password" node_ref=master_password_element/>

            <input type="submit" value="Submit"/>
        </form>
    }
}

fn main() {
    leptos::mount_to_body(App)
}
