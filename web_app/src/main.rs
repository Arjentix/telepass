//! Web App service which provides the frontend for Telegram Bot.

// Triggered by leptos
#![allow(clippy::empty_structs_with_brackets)]
#![allow(clippy::same_name_method)]

use js_sys::Reflect;
use leptos::*;
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
    /// Only up to 4096 bytes of `data` will be sent.
    #[wasm_bindgen(method)]
    fn sendData(this: &WebApp, data: JsValue);
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

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default(); // Prevent page reload
        web_app.sendData("Submitted".into());
    };

    view! {
        <form on:submit=on_submit>
            <label for="resource_name">Resource name</label>
            <input type="text" id="resource_name"/>

            <label for="password">Password</label>
            <input type="password" id="password"/>

            <label for="master-password">Master Password</label>
            <input type="password" id="master-password"/>

            <input type="submit" value="Submit"/>
        </form>
    }
}

fn main() {
    leptos::mount_to_body(App)
}
