# Standalone Dockerfile usage:
#
# Dev build: `docker build .. -f Dockerfile --target dev-runtime -t telepass/telegram_gate:dev`
# Prod build: `docker build .. -f Dockerfile -t telepass/telegram_gate`

ARG SERVICE_NAME=telegram_gate
ARG FULL_SERVICE_NAME=telepass_${SERVICE_NAME}
ARG SERVICE_DIR=/usr/src/telepass/${SERVICE_NAME}


# Using cargo-chef to speed up build times
FROM lukemathwalker/cargo-chef:latest-rust-1.88 AS chef
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

RUN apt-get update && apt-get install -y protobuf-compiler

WORKDIR ${SERVICE_DIR}
COPY --from=planner /usr/src/telepass/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json --package telepass_${SERVICE_NAME}

WORKDIR /usr/src/telepass
COPY . .
WORKDIR ${SERVICE_DIR}


# Development build
FROM base-builder AS dev-builder
ARG FULL_SERVICE_NAME
RUN cargo install --locked --bin ${FULL_SERVICE_NAME} --no-default-features --features executable --path .


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

# Development runtime
FROM base-runtime AS dev-runtime
COPY --from=dev-builder /usr/local/cargo/bin/${FULL_SERVICE_NAME} /usr/local/bin/${FULL_SERVICE_NAME}

CMD exec $FULL_SERVICE_NAME


# Production ready runtime
FROM base-runtime AS prod-runtime
COPY --from=prod-builder /usr/local/cargo/bin/${FULL_SERVICE_NAME} /usr/local/bin/${FULL_SERVICE_NAME}

EXPOSE 50051
CMD exec $FULL_SERVICE_NAME
