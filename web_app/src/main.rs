//! Web App service which provides the frontend for Telegram Bot.

// Triggered by leptos
#![allow(clippy::empty_structs_with_brackets)]
#![allow(clippy::same_name_method)]

use js_sys::Reflect;
use leptos::*;
use wasm_bindgen::prelude::*;

// #[wasm_bindgen(module = "https://telegram.org/js/telegram-web-app.js")]
#[wasm_bindgen]
extern "C" {
    type WebApp;

    #[wasm_bindgen(method)]
    fn expand(this: &WebApp);
}

/// Main component.
#[allow(
    clippy::missing_docs_in_private_items,
    clippy::panic,
    clippy::expect_used
)]
#[component]
fn App() -> impl IntoView {
    let Some(window) = web_sys::window() else {
        panic!("No window found");
    };
    let Ok(telegram) = Reflect::get(&window, &JsValue::from_str("Telegram")) else {
        panic!("No Telegram found in window");
    };
    let Ok(web_app) = Reflect::get(&telegram, &JsValue::from_str("WebApp")) else {
        panic!("No WebApp found in window.Telegram");
    };

    // `WebApp` is not a class, so checked casts like `dyn_into` fail.
    let web_app = web_app.unchecked_into::<WebApp>();
    web_app.expand();

    let (count, set_count) = create_signal(0_u32);

    view! {
        <button
            on:click=move |_| {
                set_count.update(|n| *n = n.wrapping_add(1));
            }
        >
            "Click me"
        </button>
        <p>
            <strong>"Count: "</strong>
            {count}
        </p>
    }
}

fn main() {
    leptos::mount_to_body(App)
}
