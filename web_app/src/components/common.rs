//! Common sub-components.

use leptos::{
    component, create_node_ref, create_signal,
    html::{ElementDescriptor, Input, Textarea},
    view, Children, IntoView, NodeRef, ReadSignal, WriteSignal,
};
use serde::{Deserialize, Serialize};
use web_sys::SubmitEvent;

const PASSWORD_TY: &str = "password";
const TEXT_TY: &str = "text";
const EYE_CLASS: &str = "fas fa-eye";
const SLASHED_EYE_CLASS: &str = "fas fa-eye-slash";

/// Payload with user data to be encrypted and sent to the bot backend.
#[derive(Serialize, Deserialize)]
pub struct Payload {
    /// Resource name.
    pub resource_name: String,
    /// Login.
    pub login: String,
    /// Password.
    pub password: String,
    /// Any additional comments.
    pub comments: String,
}

/// Parameter of [`RecordForm`] component describing how element should be shown.
pub struct RecordFormParamRead<T: ElementDescriptor + 'static> {
    /// Value of the element.
    pub value: ReadSignal<String>,
    /// Whether the element is read-only.
    pub readonly: ReadSignal<bool>,
    /// Reference to the element.
    pub element: NodeRef<T>,
}

impl<T: ElementDescriptor + 'static> Copy for RecordFormParamRead<T> {}

impl<T: ElementDescriptor + 'static> Clone for RecordFormParamRead<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Write-part of [`RecordFormParamRead`] to change values reactively.
pub struct RecordFormParamWrite {
    /// Value of the element.
    pub value: WriteSignal<String>,
    /// Whether the element is read-only.
    pub _readonly: WriteSignal<bool>,
}

/// Create a parameter for the record form together with the write-part.
pub fn create_record_form_parameter<T: ElementDescriptor + 'static>(
    value: String,
    readonly: bool,
) -> (RecordFormParamRead<T>, RecordFormParamWrite) {
    let (value, set_value) = create_signal(value);
    let (readonly, set_readonly) = create_signal(readonly);
    (
        RecordFormParamRead {
            value,
            readonly,
            element: create_node_ref::<T>(),
        },
        RecordFormParamWrite {
            value: set_value,
            _readonly: set_readonly,
        },
    )
}

/// Main component with the record form.
#[component]
pub fn RecordForm<F: Fn(SubmitEvent) + 'static>(
    resource_name: RecordFormParamRead<Input>,
    login: RecordFormParamRead<Input>,
    password: RecordFormParamRead<Input>,
    comments: RecordFormParamRead<Textarea>,
    master_password_element: NodeRef<Input>,
    submit_value: &'static str,
    on_submit: F,
) -> impl IntoView {
    let resource_name_element = resource_name.element;
    let login_element = login.element;
    let comments_element = comments.element;

    let (password_ty, set_password_ty) = create_signal(PASSWORD_TY);
    let (master_password_ty, set_master_password_ty) = create_signal(PASSWORD_TY);

    view! {
        <form on:submit=on_submit class="submit-form">
            <label for="resource_name">Resource name</label>
            <input type="text" id="resource_name" prop:value=resource_name.value readonly=resource_name.readonly node_ref=resource_name_element/>

            <label for="login">Login</label>
            <input type="text" id="login" prop:value=login.value readonly=login.readonly node_ref=login_element/>

            <label for="password">Password</label>
            <VisibilityToggle set_ty=set_password_ty>
                <input type=password_ty id="password" prop:value=password.value readonly=password.readonly/>
            </VisibilityToggle>

            <details>
                <summary>Comments</summary>
                <textarea id="comments" prop:value=comments.value readonly=comments.readonly node_ref=comments_element/>
            </details>

            <label for="master-password">Master Password</label>
            <VisibilityToggle set_ty=set_master_password_ty>
                <input type=master_password_ty id="master-password" node_ref=master_password_element/>
            </VisibilityToggle>

            <input type="submit" value=submit_value/>
        </form>
    }
}

/// Component to toggle password visibility.
///
/// Initial visibility is expected to be hidden.
#[component]
fn VisibilityToggle(set_ty: WriteSignal<&'static str>, children: Children) -> impl IntoView {
    let (toggle_class, set_toggle_class) = create_signal(EYE_CLASS);
    let mut toggled = false;

    let on_password_toggle_click = move |_event| {
        let (new_password_ty, new_toggle_class) = if toggled {
            (PASSWORD_TY, EYE_CLASS)
        } else {
            (TEXT_TY, SLASHED_EYE_CLASS)
        };
        toggled = !toggled;

        set_ty(new_password_ty);
        set_toggle_class(new_toggle_class);
    };

    view! {
        <div class="password-box">
            {children()}
            <span class="password-toggle-icon">
                <i class=toggle_class on:click=on_password_toggle_click/>
            </span>
        </div>
    }
}
