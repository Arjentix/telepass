//! Common sub-components.

use leptos::{
    component, create_node_ref, create_signal,
    html::{ElementDescriptor, Input, Textarea},
    view, Children, IntoView, NodeRef, ReadSignal, WriteSignal,
};
use serde::{Deserialize, Serialize};
use web_sys::SubmitEvent;

mod css {
    //! Module with css classes

    /// Type of password field protected with stars.
    pub const PASSWORD_TY: &str = "password";
    /// Type of password field for unprotected text view.
    pub const TEXT_TY: &str = "text";
    /// Class of an open eye behind password field.
    pub const EYE_CLASS: &str = "fas fa-eye";
    /// Class of a slashed eye behind password field.
    pub const SLASHED_EYE_CLASS: &str = "fas fa-eye-slash";
}

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
    /// Name of the resource.
    resource_name: RecordFormParamRead<Input>,
    /// Login.
    login: RecordFormParamRead<Input>,
    /// Password.
    password: RecordFormParamRead<Input>,
    /// Comments.
    comments: RecordFormParamRead<Textarea>,
    /// Master password.
    master_password_element: NodeRef<Input>,
    /// If copy buttons for input fields are enabled.
    copy_buttons_enabled: bool,
    /// String on the submit button.
    submit_value: &'static str,
    /// Callback, which will be called when user presses the submit button.
    on_submit: F,
) -> impl IntoView {
    let resource_name_element = resource_name.element;
    let login_element = login.element;
    let password_element = password.element;
    let comments_element = comments.element;

    let (password_ty, set_password_ty) = create_signal(css::PASSWORD_TY);
    let (master_password_ty, set_master_password_ty) = create_signal(css::PASSWORD_TY);

    view! {
        <form on:submit=on_submit class="record-form">
            <FormItem>
                <label for="resource_name">Resource name</label>
                <InputBox>
                    <input type="text" id="resource_name" prop:value=resource_name.value
                        readonly=resource_name.readonly node_ref=resource_name_element
                        autocapitalize="false" autocorrect="false" spellcheck="false"/>
                </InputBox>
            </FormItem>

            <FormItem>
                <label for="login">Login</label>
                <InputBox>
                    <Copyable enabled=copy_buttons_enabled node_ref=login_element>
                        <input type="text" id="login" prop:value=login.value readonly=login.readonly
                            node_ref=login_element autocapitalize="false" autocorrect="false"
                            spellcheck="false"/>
                        <div class="invisible-button-placeholder"/>
                    </Copyable>
                </InputBox>
            </FormItem>

            <FormItem>
                <label for="password">Password</label>
                <InputBox>
                    <Copyable enabled=copy_buttons_enabled node_ref=password_element>
                        <VisibilityToggle set_ty=set_password_ty>
                            <input type=password_ty id="password" prop:value=password.value
                                readonly=password.readonly node_ref=password_element autocapitalize="false"
                                autocorrect="false" spellcheck="false"/>
                        </VisibilityToggle>
                    </Copyable>
                </InputBox>
            </FormItem>

            <FormItem>
                <details>
                    <summary>Comments</summary>
                    <textarea id="comments" prop:value=comments.value readonly=comments.readonly
                        node_ref=comments_element autocapitalize="false" autocorrect="false"
                        spellcheck="false"/>
                </details>
            </FormItem>

            <FormItem>
                <label for="master-password">Master Password</label>
                <InputBox>
                    <VisibilityToggle set_ty=set_master_password_ty>
                        <input type=master_password_ty id="master-password" node_ref=master_password_element
                            autocapitalize="false" autocorrect="false" spellcheck="false"/>
                    </VisibilityToggle>
                </InputBox>
            </FormItem>

            <FormItem>
                <input type="submit" value=submit_value/>
            </FormItem>
        </form>
    }
}

/// Component for form items like login, password and etc.
#[component]
fn FormItem(
    /// Child component to render inside form item.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="form-item">
            {children()}
        </div>
    }
}

/// Input box for login, password and etc.
#[component]
fn InputBox(
    /// Child component to render inside input box.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="input-box">
            {children()}
        </div>
    }
}

/// Component which adds a copy button to its children to copy the data inside.
#[component]
fn Copyable(
    /// If copy enabled or not.
    enabled: bool,
    /// Reference to a node to copy data from.
    node_ref: NodeRef<Input>,
    /// Child component to add copy button to.
    children: Children,
) -> impl IntoView {
    let on_copy_click = move |_event| {
        let clipboard = web_sys::window()
            .expect("No window found")
            .navigator()
            .clipboard();

        let value = node_ref
            .get()
            .map(|element| element.value())
            .unwrap_or_default();
        let promise = wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&value));

        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = promise.await {
                web_sys::console::log_2(&"failed to copy to clipboard".into(), &err);
            }
        });
    };

    let copy_button = move || {
        enabled.then(|| {
            view! {
                <button type="button" class="copy-button" on:click=on_copy_click>
                    <i class="far fa-copy"/>
                </button>
            }
        })
    };

    view! {
        <div class="copyable-box">
            {children()}
            {copy_button()}
        </div>
    }
}

/// Component to toggle password visibility, adds an eye icon.
///
/// Initial visibility is expected to be hidden.
#[component]
fn VisibilityToggle(
    /// Writer to send visibility css types.
    set_ty: WriteSignal<&'static str>,
    /// Child component to add eye icon to.
    children: Children,
) -> impl IntoView {
    let (toggle_class, set_toggle_class) = create_signal(css::EYE_CLASS);
    let mut toggled = false;

    let on_password_toggle_click = move |_event| {
        let (new_password_ty, new_toggle_class) = if toggled {
            (css::PASSWORD_TY, css::EYE_CLASS)
        } else {
            (css::TEXT_TY, css::SLASHED_EYE_CLASS)
        };
        toggled = !toggled;

        set_ty(new_password_ty);
        set_toggle_class(new_toggle_class);
    };

    view! {
        <div class="password-box">
            {children()}
            <button type="button" class="password-toggle-button" on:click=on_password_toggle_click>
                <i class=toggle_class/>
            </button>
        </div>
    }
}
