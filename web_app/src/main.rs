//! Web App service which provides the frontend for Telegram Bot.

// Triggered by leptos
#![allow(clippy::empty_structs_with_brackets)]
#![allow(clippy::same_name_method)]

use js_sys::Reflect;
use leptos::{html::Input, *};
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

/// Error during new password submission.
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
enum SubmissionError {
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
#[allow(
    clippy::missing_docs_in_private_items,
    clippy::panic,
    clippy::expect_used
)]
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
    web_app.enableClosingConfirmation();

    let resource_name_element = create_node_ref::<Input>();
    let password_element = create_node_ref::<Input>();
    let master_password_element = create_node_ref::<Input>();

    let (submission_result, set_submit_result) = create_signal(Ok(()));

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default(); // Prevent page reload

        let resource = resource_name_element()
            .expect("No resource_name element")
            .value();
        let password = password_element().expect("No password element").value();
        let master_password = master_password_element()
            .expect("No master_password element")
            .value();
        set_submit_result(|| -> Result<(), SubmissionError> {
            let telepass_crypto::EncryptionOutput {
                encrypted_password,
                salt,
            } = telepass_crypto::encrypt(&password, &master_password)?;

            let new_password = telepass_data_model::NewPassword {
                resource,
                encrypted_password,
                salt,
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

            <label for="password">Password</label>
            <input type="password" id="password" node_ref=password_element/>

            <label for="master-password">Master Password</label>
            <input type="password" id="master-password" node_ref=master_password_element/>

            <input type="submit" value="Submit"/>
        </form>
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

fn main() {
    leptos::mount_to_body(App)
}
