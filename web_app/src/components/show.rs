//! Module with [`Show`] component implementation.

use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use leptos::{
    component, create_node_ref, html::Input, view, IntoView, Params, SignalGetUntracked as _,
    SignalSet as _, WriteSignal,
};
use leptos_router::{use_query, Params, ParamsError};
use web_sys::SubmitEvent;

use super::common::{create_record_form_parameter, Payload, RecordForm};

/// Error during record presentation.
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
pub enum Error {
    /// Failed to parse query
    ParamsParsing(#[from] ParamsError),
    /// Both payload and salt must be provided
    MissingParams,
    /// Failed to decode payload or salt
    Base64Decoding(#[from] base64::DecodeError),
    /// Wrong salt length
    WrongSaltLength,
    /// Failed to decrypt data
    Decryption(#[from] telepass_crypto::Error),
    /// Failed to deserialize data: {0}
    Deserialization(String),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Deserialization(e.to_string())
    }
}

#[derive(Params, Clone, PartialEq, Eq)]
struct QueryParams {
    payload: Option<String>,
    salt: Option<String>,
}

#[component]
pub fn Show(set_result: WriteSignal<Result<(), Error>>) -> impl IntoView {
    let payload_and_salt = use_query::<QueryParams>()
        .get_untracked()
        .map_err(Error::from)
        .and_then(|params| match (params.payload, params.salt) {
            (Some(payload), Some(salt)) => Ok((payload, salt)),
            _ => Err(Error::MissingParams),
        })
        .and_then(|(url_encoded_payload, url_encoded_salt)| {
            URL_SAFE
                .decode(url_encoded_payload)
                .and_then(|payload| {
                    URL_SAFE
                        .decode(url_encoded_salt)
                        .map(|salt| (payload, salt))
                })
                .map_err(Into::into)
        })
        .and_then(|(payload, salt)| {
            Ok((
                payload,
                salt.try_into().map_err(|_err| Error::WrongSaltLength)?,
            ))
        })
        .map_err(|err| {
            set_result(Err(err));
        })
        .ok();

    let (resource_name, set_resource_name) = create_record_form_parameter(String::new(), true);
    let (login, set_login) = create_record_form_parameter(String::new(), true);
    let (password, set_password) = create_record_form_parameter(String::new(), true);
    let (comments, set_comments) = create_record_form_parameter(String::new(), true);
    let master_password_element = create_node_ref::<Input>();

    let on_decrypt = move |event: SubmitEvent| {
        event.prevent_default(); // Prevent page reload

        let Some((payload, salt)) = payload_and_salt.clone() else {
            return;
        };

        let master_password = master_password_element()
            .expect("No master_password element")
            .value();

        let payload = telepass_crypto::decrypt(
            telepass_crypto::EncryptionOutput {
                encrypted_payload: payload,
                salt,
            },
            &master_password,
        )
        .map_err(Error::from)
        .and_then(|record| serde_json::from_str::<Payload>(&record).map_err(Into::into));

        let payload = match payload {
            Ok(payload) => {
                set_result(Ok(()));
                payload
            }
            Err(err) => {
                set_result(Err(err));
                return;
            }
        };
        set_resource_name.value.set(payload.resource_name);
        set_login.value.set(payload.login);
        set_password.value.set(payload.password);
        set_comments.value.set(payload.comments);
    };

    view! {
        <RecordForm
            resource_name=resource_name
            login=login
            password=password
            comments=comments
            master_password_element=master_password_element
            copy_buttons_enabled=true
            submit_value="Decrypt"
            on_submit=on_decrypt
        />
    }
}
