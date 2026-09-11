# Example Worker

The example Worker is a complete Cloudflare host for Platform's reference
application.

It compiles one application, embeds one validated configuration profile, and
adapts Cloudflare requests to the same host-neutral behavior used by the
reference native server. The application owns the `Hello` use-case. This crate
owns edge execution.

## Run locally

Install the Rust Wasm target and `worker-build` once, then install the pinned
Wrangler development dependency:

```console
rustup target add wasm32-unknown-unknown
cargo install worker-build --version 0.8.5 --locked
cd workers/example-worker
pnpm install --frozen-lockfile
```

From the Worker directory, start Wrangler:

```console
pnpm dev
```

Call the application and the Worker-owned operational endpoints:

```console
curl http://127.0.0.1:8787/hello
curl http://127.0.0.1:8787/healthz
curl http://127.0.0.1:8787/readyz
curl http://127.0.0.1:8787/version
curl http://127.0.0.1:8787/metrics
buf curl \
  --schema ../../apps/example/contract/proto \
  --protocol connect \
  --data '{}' \
  http://127.0.0.1:8787/reallyme.example.v1.ExampleService/Hello
```

The Worker embeds `apps/example/config/local.jsonc`. A production Worker should
select its own reviewed configuration source and use bindings or secrets for
values that must not be compiled into the artifact.

## Composition

The host boundary is deliberately small:

- `src/entrypoint.rs` receives Cloudflare fetch events;
- `src/routing.rs` selects application and operational routes;
- `src/app_adapter.rs` calls the host-neutral application core;
- `src/response.rs` owns Worker response encoding and security headers; and
- `wrangler.jsonc` defines the Cloudflare build and runtime configuration.

The host exposes `GET /hello` and the generated Connect RPC path for
`ExampleService.Hello`. Both call the same application use-case. Health,
readiness, version, and metrics compatibility endpoints describe this Worker
host and therefore remain outside the application core.

## Deploy

Authenticate Wrangler, review `wrangler.jsonc`, and deploy from this directory:

```console
pnpm deploy
```

The checked-in configuration enables Workers observability. Environment,
route, binding, and secret configuration remain deployment decisions; this
reference host does not invent production resources.

Validate the build without uploading an artifact:

```console
pnpm worker:check
```

## Validate

From the Platform repository root:

```console
cargo fmt --all -- --check
cargo check -p example-worker
cargo check -p example-worker --target wasm32-unknown-unknown
cargo test -p example-worker
cargo clippy -p example-worker --all-targets --all-features -- -D warnings
cd workers/example-worker
pnpm audit --audit-level=high
pnpm worker:check
```

## Boundary rules

- Keep product behavior in the application, not this host.
- Keep `worker` and Cloudflare runtime types out of application core.
- Do not import `reallyme-server-kit`; Cloudflare owns the edge lifecycle.
- Validate embedded config before constructing application context.
- Return typed success bodies and stable public error envelopes.
- Apply Cloudflare-specific headers, bindings, and routing policy here.
- Keep HSTS in Cloudflare zone or origin policy so local plaintext development
  remains valid.
