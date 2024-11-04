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

/// Components result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Deserialization(e.to_string())
    }
}

/// Query parameters for `/show` url.
#[derive(Clone)]
struct QueryParams {
    /// Name of the displayed resource.
    resource_name: Option<String>,
    /// Encrypted payload with password and etc.
    payload: Vec<u8>,
    /// Salt used for encryption.
    salt: telepass_crypto::Salt,
}

/// [`QueryParams`] candidate which is easy to parse.
#[derive(Params, Clone, PartialEq, Eq)]
struct QueryParamsCandidate {
    /// Name of the displayed resource.
    resource_name: Option<String>,
    /// Encrypted payload with password and etc.
    payload: Option<String>,
    /// Salt used for encryption.
    salt: Option<String>,
}

impl QueryParams {
    /// Parse [`QueryParams`] from url.
    fn parse_from_url() -> Result<Self> {
        let candidate = use_query::<QueryParamsCandidate>().get_untracked()?;

        let (Some(payload), Some(salt)) = (candidate.payload, candidate.salt) else {
            return Err(Error::MissingParams);
        };

        let payload = URL_SAFE.decode(payload)?;

        let salt = URL_SAFE.decode(salt)?;
        let salt = salt.try_into().map_err(|_err| Error::WrongSaltLength)?;

        Ok(Self {
            resource_name: candidate.resource_name,
            payload,
            salt,
        })
    }
}

#[component]
pub fn Show(
    /// Writer to set the result of user action.
    set_result: WriteSignal<Result<()>>,
) -> impl IntoView {
    let query_params = QueryParams::parse_from_url()
        .map_err(|err| {
            set_result(Err(err));
        })
        .ok();

    let (resource_name, set_resource_name) = create_record_form_parameter(
        query_params
            .as_ref()
            .and_then(|params| params.resource_name.clone())
            .unwrap_or_default(),
        true,
    );
    let (login, set_login) = create_record_form_parameter(String::new(), true);
    let (password, set_password) = create_record_form_parameter(String::new(), true);
    let (comments, set_comments) = create_record_form_parameter(String::new(), true);
    let master_password_element = create_node_ref::<Input>();

    let on_decrypt = move |event: SubmitEvent| {
        event.prevent_default(); // Prevent page reload

        let Some(query_params) = query_params.clone() else {
            return;
        };

        let master_password = master_password_element()
            .expect("No master_password element")
            .value();

        let payload = telepass_crypto::decrypt(
            telepass_crypto::EncryptionOutput {
                encrypted_payload: query_params.payload,
                salt: query_params.salt,
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
