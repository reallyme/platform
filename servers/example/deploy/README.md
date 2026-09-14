# Example Server Deployment

Build the reference container from the Platform repository root so Cargo can
see the application and kit crates in the workspace:

```console
docker build -f servers/example/deploy/Dockerfile -t example-server:local .
docker run --rm -p 127.0.0.1:8080:8080 example-server:local
```

The container config binds the process to its container interface, while the
example command publishes that port only on the host's loopback interface.
Real deployments should replace the example document with configuration that
deliberately selects the bind address, trusted proxy policy, allowed hosts, and
operational-route exposure for their ingress topology.
