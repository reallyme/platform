# syntax=docker/dockerfile:1.7

FROM rust:1.96-bookworm AS builder
WORKDIR /workspace

COPY . .
RUN cargo build --locked --release -p example-server

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /workspace/target/release/example-server \
    /usr/local/bin/example-server
COPY servers/example-server/deploy/example-server.container.jsonc \
    /etc/reallyme/example-server.jsonc

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/example-server", "--config", "/etc/reallyme/example-server.jsonc"]
