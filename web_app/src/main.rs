//! Web App service which provides the frontend for Telegram Bot.

#![allow(
    clippy::empty_structs_with_brackets,
    clippy::same_name_method,
    clippy::missing_docs_in_private_items,
    reason = "triggered by leptos"
)]
#![allow(clippy::panic, clippy::expect_used, reason = "panic in frontend is ok")]

use std::rc::Rc;

use js_sys::Reflect;
use leptos::{
    component, create_signal, view, CollectView as _, ErrorBoundary, IntoView, SignalGet as _,
};
use leptos_router::*;
use wasm_bindgen::prelude::*;

mod components;
mod tg_api;

/// Main component.
#[component]
fn App() -> impl IntoView {
    let window = web_sys::window().expect("No window found");
    let telegram =
        Reflect::get(&window, &JsValue::from_str("Telegram")).expect("No Telegram found in window");
    let web_app = Reflect::get(&telegram, &JsValue::from_str("WebApp"))
        .expect("No WebApp found in window.Telegram");

    // `WebApp` is not a class, so checked casts like `dyn_into` fail.
    let web_app = web_app.unchecked_into::<tg_api::WebApp>();
    web_app.expand();

    let web_app = Rc::new(web_app);

    let (submission_result, set_submission_result) = create_signal(Ok(()));
    let (show_result, set_show_result) = create_signal(Ok(()));

    view! {
        <Router>
            <Routes>
                <Route path="/submit" view=move || view! {
                    <components::Submit web_app=Rc::clone(&web_app) set_result=set_submission_result/>
                }/>
                <Route path="/show" view=move || view! {
                    <components::Show set_result=set_show_result/>
                }/>
                <Route path="/*any" view=|| view! { <h1>"Not Found"</h1> }/>
            </Routes>
        </Router>
        <ErrorBoundary fallback=|errors| view! {
            <div class="error-container">
                <div class="error">
                    { move || {
                        errors.get()
                        .into_iter()
                        .map(|(_, e)| view! { <p>{e.to_string()}</p>})
                        .collect_view()
                    }}
                </div>
            </div>
        }>
            { submission_result }
            { show_result }
        </ErrorBoundary>
    }
}

fn main() {
    leptos::mount_to_body(App)
}
