// @generated automatically by Diesel CLI.

diesel::table! {
    passwords (resource_name) {
        resource_name -> Varchar,
        encrypted_payload -> Bytea,
        salt -> Bytea,
    }
}
