CREATE TABLE passwords (
  resource_name VARCHAR(255) PRIMARY KEY,
  encrypted_payload BYTEA NOT NULL,
  salt BYTEA NOT NULL
);
