ARG SERVICE_NAME=db_service


# Using cargo-chef to speed up build times
FROM lukemathwalker/cargo-chef:latest-rust-1.67 AS chef
WORKDIR /usr/src/telepass/${SERVICE_NAME}


# Preparing cache with cargo-chef
FROM chef AS planner

ARG SERVICE_NAME

WORKDIR /usr/src/telepass
COPY ./${SERVICE_NAME} ./${SERVICE_NAME}

WORKDIR /usr/src/telepass/${SERVICE_NAME}
RUN cargo chef prepare --recipe-path recipe.json


# Building the actual image
FROM chef AS builder

ARG SERVICE_NAME

RUN apt-get update && apt-get install -y protobuf-compiler

COPY --from=planner /usr/src/telepass/${SERVICE_NAME}/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY ./proto ./proto
COPY ./${SERVICE_NAME} ./${SERVICE_NAME}

WORKDIR /usr/src/telepass/${SERVICE_NAME}
RUN cargo install --path .


# Running the service
FROM debian:bullseye-slim AS runtime

ARG SERVICE_NAME
ENV FULL_SERVICE_NAME=telepass_${SERVICE_NAME}

RUN apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/telepass/${SERVICE_NAME}
COPY --from=builder /usr/src/telepass/${SERVICE_NAME}/.env .env
COPY --from=builder /usr/src/telepass/${SERVICE_NAME}/diesel.toml diesel.toml
COPY --from=builder /usr/local/cargo/bin/${FULL_SERVICE_NAME} /usr/local/bin/${FULL_SERVICE_NAME}

EXPOSE 50051
CMD $FULL_SERVICE_NAME
