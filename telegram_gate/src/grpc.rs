//! Module with `gRPC` client for `password_storage` service

#![expect(
    clippy::empty_structs_with_brackets,
    clippy::missing_errors_doc,
    clippy::future_not_send,
    clippy::allow_attributes_without_reason,
    clippy::derive_partial_eq_without_eq,
    reason = "generated code"
)]
#![allow(
    clippy::missing_docs_in_private_items,
    unfulfilled_lint_expectations,
    reason = "allow because it's false positive unfulfilled-lint-expectations if in expect"
)]

tonic::include_proto!("password_storage");

#[cfg(test)]
mockall::mock! {
    pub PasswordStorageClient {
        // Copy-paste from `include_proto!` macro expansion.
        // Unfortunately there is no better way to mock this.

        pub async fn add<R: tonic::IntoRequest<Record> + 'static>(
            &mut self,
            request: R
        ) -> Result<tonic::Response<Response>, tonic::Status>;

        pub async fn delete<R: tonic::IntoRequest<Resource> + 'static>(
            &mut self,
            request: R
        ) -> Result<tonic::Response<Response>, tonic::Status>;

        pub async fn get<R: tonic::IntoRequest<Resource> + 'static>(
            &mut self,
            request: R
        ) -> Result<tonic::Response<Record>, tonic::Status>;

        pub async fn list<R: tonic::IntoRequest<Empty> + 'static>(
            &mut self,
            request: R
        ) -> Result<tonic::Response<ListOfResources>, tonic::Status>;

        pub async fn search<R: tonic::IntoRequest<Resource> + 'static>(
            &mut self,
            request: R,
        ) -> Result<tonic::Response<ListOfResources>, tonic::Status>;
    }
}

impl From<telepass_data_model::NewRecord> for Record {
    fn from(record: telepass_data_model::NewRecord) -> Self {
        Self {
            resource: Some(Resource {
                name: record.resource_name,
            }),
            encrypted_payload: record.encryption_output.encrypted_payload,
            salt: record.encryption_output.salt.to_vec(),
        }
    }
}
