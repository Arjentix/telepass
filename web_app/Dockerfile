# Standalone Dockerfile usage:
#
# `docker build .. -f Dockerfile -t telepass/web_app`

FROM rust:1.88 as builder

RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk

WORKDIR /usr/src/telepass
COPY . .

RUN cd web_app && trunk build --release


FROM nginx:1.25-alpine

COPY ./web_app/nginx.conf /etc/nginx/nginx.conf
COPY --from=builder /usr/src/telepass/web_app/dist/ /usr/share/nginx/html/

EXPOSE 80
EXPOSE 443
CMD exec nginx -g 'daemon off;'
