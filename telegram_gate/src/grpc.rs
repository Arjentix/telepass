//! Module with `gRPC` client for `password_storage` service

#![allow(clippy::empty_structs_with_brackets)]
#![allow(clippy::similar_names)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::future_not_send)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::indexing_slicing)] // Triggered by `mock!`

tonic::include_proto!("password_storage");

#[cfg(test)]
mockall::mock! {
    pub PasswordStorageClient {
        #[allow(clippy::used_underscore_binding)]
        pub fn new(_channel: tonic::transport::Channel) -> Self {
            unreachable!()
        }

        // Copy-paste from `include_proto!` macro expansion.
        // Unfortunately there is no better way to mock this.

        pub async fn add<R: tonic::IntoRequest<Record> + 'static>(
            &mut self,
            request: R
        ) -> std::result::Result<tonic::Response<Response>, tonic::Status>;

        pub async fn delete<R: tonic::IntoRequest<Resource> + 'static>(
            &mut self,
            request: R
        ) -> std::result::Result<tonic::Response<Response>, tonic::Status>;

        pub async fn get<R: tonic::IntoRequest<Resource> + 'static>(
            &mut self,
            request: R
        ) -> std::result::Result<tonic::Response<Record>, tonic::Status>;

        pub async fn list<R: tonic::IntoRequest<Empty> + 'static>(
            &mut self,
            request: R
        ) -> std::result::Result<tonic::Response<ListOfResources>, tonic::Status>;
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
