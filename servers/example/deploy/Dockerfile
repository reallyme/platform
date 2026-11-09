# syntax=docker/dockerfile:1.28@sha256:bb22d9815c728170f72750f4e5b0d672e06176142e1d602c7e66c050100b7e5b

FROM rust:1.99.1-bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS builder
WORKDIR /workspace

COPY . .
RUN cargo build --locked --release -p example-server

FROM gcr.io/distroless/cc-debian12:nonroot@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f
COPY --from=builder /workspace/target/release/example-server \
    /usr/local/bin/example-server
COPY servers/example/deploy/example-server.container.jsonc \
    /etc/reallyme/example-server.jsonc

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/example-server", "--config", "/etc/reallyme/example-server.jsonc"]
