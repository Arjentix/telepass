//! Module with [`Submit`] component implementation.

use std::rc::Rc;

use leptos::{
    component, create_node_ref,
    html::{Input, Textarea},
    view, IntoView, WriteSignal,
};
use web_sys::SubmitEvent;

use super::common::{create_record_form_parameter, Payload, RecordForm};
use crate::tg_api::WebApp;

/// Error during new password submission.
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
pub enum Error {
    /// Invalid input: {0}
    Validation(&'static str),
    /// Failed to encrypt data
    Encryption(#[from] telepass_crypto::Error),
    /// Failed to serialize data: {0}
    Serialization(String),
    /// Failed to send data to the bot backend: {0}
    Sending(String),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

/// Component with input forms and `Submit` button.
///
/// Clicking on the button will send encrypted info to the bot via `web_app` and close the app.
#[component]
pub fn Submit(web_app: Rc<WebApp>, set_result: WriteSignal<Result<(), Error>>) -> impl IntoView {
    web_app.enableClosingConfirmation();

    let (resource_name, _set_resource_name) =
        create_record_form_parameter::<Input>(String::new(), false);
    let (login, _set_login) = create_record_form_parameter::<Input>(String::new(), false);
    let (password, _set_password) = create_record_form_parameter::<Input>(String::new(), false);
    let (comments, _set_comments) = create_record_form_parameter::<Textarea>(String::new(), false);
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

        set_result(|| -> Result<(), Error> {
            let resource_name = resource_name.trim();
            if resource_name.is_empty() {
                return Err(Error::Validation("Resource name cannot be empty"));
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
                .map_err(|err| Error::Sending(format!("{err:?}")))
        }());
    };

    view! {
        <RecordForm
            resource_name=resource_name
            login=login
            password=password
            comments=comments
            master_password_element=master_password_element
            copy_buttons_enabled=false
            submit_value="Submit"
            on_submit=on_submit
        />
    }
}
