//! Web App service which provides the frontend for Telegram Bot.

// Triggered by leptos
#![allow(clippy::empty_structs_with_brackets)]
#![allow(clippy::same_name_method)]

use leptos::*;

/// Main component.
#[allow(clippy::missing_docs_in_private_items)]
#[component]
fn App() -> impl IntoView {
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
