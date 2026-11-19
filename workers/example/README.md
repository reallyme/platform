# Example Worker

Cloudflare Worker exposing the reference application's `GET /hello` and binary
Connect `ExampleService.Hello` RPC.

## Run locally

Install the pinned pnpm version from `package.json`, then:

```sh
rustup target add wasm32-unknown-unknown
cargo install worker-build --version '=0.9.0' --locked
cd workers/example
pnpm install --frozen-lockfile
printf "EXAMPLE_APP_CONFIG_JSONC='%s'\n" "$(jq -c . ../../apps/example/config/local.jsonc)" > .dev.vars
pnpm dev
```

The profile command requires `jq`. `EXAMPLE_APP_CONFIG_JSONC` is required;
application requests fail if the binding is missing or invalid.

From the Worker directory in another terminal:

```sh
curl http://127.0.0.1:8787/hello
buf curl --schema ../../apps/example/contract/proto --protocol connect --data '{}' \
  http://127.0.0.1:8787/reallyme.example.v1.ExampleService/Hello
```

`/healthz` and `/readyz` require an `OPERATIONAL_PROBE_TOKEN` secret binding of
32–256 bytes. Add it to `.dev.vars` locally. Set the same value in your shell
when calling a probe:

```sh
curl -H "Authorization: Bearer ${OPERATIONAL_PROBE_TOKEN}" http://127.0.0.1:8787/readyz
```

Keep `.dev.vars` out of version control. The Worker has no `/version` or
`/metrics` route; telemetry uses Workers observability. Configure HSTS in the
Cloudflare zone or origin policy.

## Deploy and check

After authenticating Wrangler, review `wrangler.jsonc`, routes, and bindings.
Provide `EXAMPLE_APP_CONFIG_JSONC` in the deployment environment and store the
probe token:

```sh
pnpm exec wrangler secret put OPERATIONAL_PROBE_TOKEN
pnpm worker:check
pnpm deploy
```

`worker:check` builds the deployment bundle without uploading it. To check the
Rust host from the repository root:

```sh
cargo check -p example-worker --target wasm32-unknown-unknown
cargo test -p example-worker
```

Keep Cloudflare request conversion and bindings in this crate. Application
behavior belongs in [example-app](../../apps/example/).
