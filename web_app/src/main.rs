//! Web App service which provides the frontend for Telegram Bot.

#![expect(
    clippy::empty_structs_with_brackets,
    clippy::same_name_method,
    reason = "triggered by leptos"
)]
#![expect(clippy::expect_used, reason = "panic in frontend is ok")]

use leptos::{
    IntoView, component,
    error::ErrorBoundary,
    prelude::{ClassAttribute as _, CollectView as _, ElementChild as _, Get as _, signal},
    view,
};
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

mod components;
mod tg_api;

/// Main component.
#[component]
fn App() -> impl IntoView {
    let web_app = tg_api::web_app();
    web_app.expand();

    let (submission_result, set_submission_result) = signal(Ok(()));
    let (show_result, set_show_result) = signal(Ok(()));

    view! {
        <Router>
            <Routes fallback=|| view! { <h1>"Not Found"</h1> }>
                <Route path=path!("/submit") view=move || view! {
                    <components::Submit set_result=set_submission_result/>
                }/>
                <Route path=path!("/show") view=move || view! {
                    <components::Show set_result=set_show_result/>
                }/>
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
    leptos::mount::mount_to_body(App)
}
