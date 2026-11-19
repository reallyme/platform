# Example server

Run the reference application from the repository root:

```sh
cargo run -p example-server -- \
  --config servers/example/config/example-server.jsonc
```

The default listener is `127.0.0.1:8080`:

```sh
curl http://127.0.0.1:8080/hello
curl http://127.0.0.1:8080/healthz
curl http://127.0.0.1:8080/readyz
curl http://127.0.0.1:8080/version
curl http://127.0.0.1:8080/metrics
```

`/hello` returns `{"message":"hello from example-app"}`. Press `Ctrl-C` to stop.
Route traffic using `/readyz`; liveness and application routes can respond
before startup completes.

The `--config` path is required. The
[server JSONC](config/example-server.jsonc) sets the listener address, request
and body limits, metrics timeout, shutdown timeout, and application profile.
Application behavior comes from [app configuration](../../apps/example/config/).
The parser uses a fixed local logging configuration and does not accept an
`observability` object.

For external ingress, configure `allowed_hosts`, trusted proxies, and
`external_origin_policy` as described in [server-kit](../../kits/server/README.md).
For a container, see [deployment](deploy/README.md).

To compose another application, add its dependency and register its adapter in
`src/registry.rs`; build the runtime in `src/composition.rs`. Configuration can
select compiled code but cannot load additional applications.

```sh
cargo test -p example-server
```
