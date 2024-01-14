CREATE TABLE passwords (
  resource_name VARCHAR(255) PRIMARY KEY,
  encrypted_payload VARCHAR(2047) NOT NULL,
  salt VARCHAR(255) NOT NULL
);
