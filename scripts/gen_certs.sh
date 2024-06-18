#!/bin/sh

# Source: https://stackoverflow.com/a/76136847/10366988

mkdir -p certs
cd certs/

# Create a self-signed root CA
openssl req -x509 -sha256 -nodes -subj "/C=RU" -days 1825 -newkey rsa:2048 -keyout root_ca.key -out root_ca.crt


# Create unencrypted private key and a CSR (certificate signing request)
openssl req -newkey rsa:2048 -nodes -subj "/C=RU" -keyout password_storage.key -out password_storage.csr

# Create self-signed certificate (`password_storage.crt`) with the private key and CSR
openssl x509 -signkey password_storage.key -in password_storage.csr -req -days 1825 -out password_storage.crt

# Create file password_storage.ext with the following content:
cat << 'EOF' >> password_storage.ext
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
subjectAltName = @alt_names
[alt_names]
DNS.1 = localhost
DNS.2 = password_storage
IP.1 = 127.0.0.1
EOF

# Sign the CSR (`password_storage.crt`) with the root CA certificate and private key
# => this overwrites `password_storage.crt` because it gets signed
openssl x509 -req -CA root_ca.crt -CAkey root_ca.key -in password_storage.csr -out password_storage.crt -days 1825 -CAcreateserial -extfile password_storage.ext


# Create unencrypted private key and a CSR (certificate signing request)
openssl req -newkey rsa:2048 -nodes -subj "/C=RU" -keyout telegram_gate.key -out telegram_gate.csr

# Create self-signed certificate (`telegram_gate.crt`) with the private key and CSR
openssl x509 -signkey telegram_gate.key -in telegram_gate.csr -req -days 1825 -out telegram_gate.crt

# Create file telegram_gate.ext with the following content:
cat << 'EOF' >> telegram_gate.ext
basicConstraints = CA:FALSE
nsCertType = client, email
nsComment = "OpenSSL Generated Client Certificate"
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
keyUsage = critical, nonRepudiation, digitalSignature, keyEncipherment
extendedKeyUsage = clientAuth, emailProtection
EOF

# Sign the CSR (`telegram_gate.crt`) with the root CA certificate and private key
# => this overwrites `telegram_gate.crt` because it gets signed
openssl x509 -req -CA root_ca.crt -CAkey root_ca.key -in telegram_gate.csr -out telegram_gate.crt -days 1825 -CAcreateserial -extfile telegram_gate.ext


# Create unencrypted private key and a CSR (certificate signing request)
openssl req -newkey rsa:2048 -nodes -subj "/C=RU" -keyout web_app_nginx.key -out web_app_nginx.csr

# Create self-signed certificate (`web_app_nginx.crt`) with the private key and CSR
openssl x509 -signkey web_app_nginx.key -in web_app_nginx.csr -req -days 1825 -out web_app_nginx.crt

# Create file web_app_nginx.ext with the following content:
cat << 'EOF' >> web_app_nginx.ext
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
subjectAltName = @alt_names
[alt_names]
DNS.1 = localhost
DNS.2 = web_app_nginx
IP.1 = 127.0.0.1
EOF

# Sign the CSR (`web_app_nginx.crt`) with the root CA certificate and private key
# => this overwrites `web_app_nginx.crt` because it gets signed
openssl x509 -trustout -req -CA root_ca.crt -CAkey root_ca.key -in web_app_nginx.csr -out web_app_nginx.crt -days 1825 -CAcreateserial -extfile web_app_nginx.ext
