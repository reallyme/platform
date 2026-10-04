# Example Server Deployment

Build the reference container from the Platform repository root so Cargo can
see the application and kit crates in the workspace:

```console
docker build -f servers/example/deploy/Dockerfile -t example-server:local .
docker run --rm -p 127.0.0.1:8080:8080 example-server:local
```

The container config uses the production app profile and accepts Host
`example.reallyme.net`. It binds the process to its container interface, while
the example command publishes that port only on the host's loopback interface.
The example document controls the bind address, allowed hosts, and bounded
timeouts. A real server host should set its own ingress hostnames and proxy
ranges in JSONC, then choose operational route exposure in its composition code
before accepting external traffic.
