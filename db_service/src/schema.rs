// @generated automatically by Diesel CLI.

diesel::table! {
    passwords (resource) {
        resource -> Varchar,
        passhash -> Varchar,
        salt -> Varchar,
    }
}
