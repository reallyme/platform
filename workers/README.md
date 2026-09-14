# Workers

`workers/` contains the reference Cloudflare Workers host for Platform.

A Worker is a deployment host, not an application. It selects an application,
owns the Cloudflare entrypoint and configuration, and translates Worker
requests into host-neutral application behavior. Product-specific Worker hosts
belong with the servers and deployments that compose them, outside the public
Platform repository.

## Reference Worker

[`example/`](example/) runs the reference application on
Cloudflare Workers. It demonstrates the complete edge boundary:

- a `workers-rs` fetch entrypoint;
- Wrangler configuration and Wasm compilation;
- explicit selection of one application and one checked-in config profile;
- HTTP and Connect RPC request adaptation;
- stable public errors and host-owned security headers; and
- Worker-local health, readiness, version, and metrics compatibility routes.

The same application core also runs inside the reference native server. The
application owns behavior; each host owns execution.

## Boundary

Worker hosts may depend on `worker` and other Cloudflare APIs. Application
cores and `reallyme-app-kit` must not.

Worker hosts do not use `reallyme-server-kit`: Cloudflare owns the runtime
lifecycle, while the Worker owns its entrypoint, routing, bindings, and
platform-specific response policy. Listeners, process shutdown, and native
readiness supervision do not exist at this boundary.

See the [example Worker README](example/) for local development,
deployment, and validation commands.
