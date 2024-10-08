# Standalone Dockerfile usage:
#
# Dev build: `docker build .. -f Dockerfile --target dev-runtime -t telepass/password_storage:dev`
# Prod build: `docker build .. -f Dockerfile -t telepass/password_storage`

ARG SERVICE_NAME=password_storage
ARG FULL_SERVICE_NAME=telepass_${SERVICE_NAME}
ARG SERVICE_DIR=/usr/src/telepass/${SERVICE_NAME}


# Using cargo-chef to speed up build times
FROM lukemathwalker/cargo-chef:latest-rust-1.80 AS chef
ARG SERVICE_DIR
WORKDIR ${SERVICE_DIR}


# Preparing cache with cargo-chef
FROM chef AS planner

ARG SERVICE_NAME
ARG SERVICE_DIR

COPY . /usr/src/telepass

WORKDIR /usr/src/telepass
RUN cargo chef prepare --recipe-path recipe.json


# Base builder for further prod and dev builds
FROM chef AS base-builder

ARG SERVICE_NAME
ARG SERVICE_DIR

RUN apt-get update && apt-get install -y lld protobuf-compiler
RUN cargo install diesel_cli --version 2.2.1 --no-default-features --features postgres

WORKDIR /usr/share/
# TODO: Keep until https://github.com/onatm/grpc-health-probe-rs/pull/1 is merged
RUN git clone https://github.com/Arjentix/grpc-health-probe-rs.git
WORKDIR /usr/share/grpc-health-probe-rs
RUN cargo install --path .

WORKDIR ${SERVICE_DIR}
COPY --from=planner /usr/src/telepass/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json --package telepass_${SERVICE_NAME}

WORKDIR /usr/src/telepass
COPY . .
WORKDIR ${SERVICE_DIR}


# Development build
FROM base-builder AS dev-builder
ARG FULL_SERVICE_NAME
RUN cargo install --locked --bin ${FULL_SERVICE_NAME} --no-default-features --features "executable, development" --path .


# Production ready build
FROM base-builder AS prod-builder
ARG FULL_SERVICE_NAME
RUN cargo install --locked --bin ${FULL_SERVICE_NAME} --features executable --path .


# --- RUNTIME ---


# Base runtime for further prod and dev runtimes
FROM debian:bookworm-slim AS base-runtime

ARG SERVICE_NAME
ARG SERVICE_DIR
ARG FULL_SERVICE_NAME
ENV FULL_SERVICE_NAME=${FULL_SERVICE_NAME}

RUN apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*

WORKDIR ${SERVICE_DIR}
COPY --from=base-builder ${SERVICE_DIR}/diesel.toml diesel.toml
COPY --from=base-builder ${SERVICE_DIR}/migrations migrations
COPY --from=base-builder /usr/local/cargo/bin/diesel /usr/local/bin/diesel
COPY --from=base-builder /usr/local/cargo/bin/grpc_health_probe /usr/local/bin/grpc_health_probe


# Development runtime
FROM base-runtime AS dev-runtime
COPY --from=dev-builder /usr/local/cargo/bin/${FULL_SERVICE_NAME} /usr/local/bin/${FULL_SERVICE_NAME}

HEALTHCHECK --interval=5s --timeout=5s --start-period=5s --retries=3 \
    CMD grpc_health_probe --addr=http://localhost:50051 \
    --service password_storage.PasswordStorage \
    || exit 1

EXPOSE 50051
CMD diesel setup && diesel migration run && exec $FULL_SERVICE_NAME


# Production ready runtime
FROM base-runtime AS prod-runtime

ENV ROOT_CA_CERT_PATH=""
ENV GRPC_HEALTH_PROBE_TLS_CERT_PATH=""
ENV GRPC_HEALTH_PROBE_TLS_KEY_PATH=""

COPY --from=prod-builder /usr/local/cargo/bin/${FULL_SERVICE_NAME} /usr/local/bin/${FULL_SERVICE_NAME}

HEALTHCHECK --interval=5s --timeout=5s --start-period=5s --retries=3 \
    CMD grpc_health_probe --addr=https://localhost:50051 --tls \
    --tls-ca-cert ${ROOT_CA_CERT_PATH} \
    --tls-client-cert ${GRPC_HEALTH_PROBE_TLS_CERT_PATH} \
    --tls-client-key ${GRPC_HEALTH_PROBE_TLS_KEY_PATH} \
    --service password_storage.PasswordStorage \
    || exit 1

EXPOSE 50051
CMD diesel setup && diesel migration run && exec $FULL_SERVICE_NAME
