# Example server container

Build and run from the repository root:

```sh
docker build -f servers/example/deploy/Dockerfile -t example-server:local .
docker run --rm -p 127.0.0.1:8080:8080 example-server:local
```

The container listens on `0.0.0.0:8080`, uses the production app profile, and
accepts Host `example.reallyme.net`. Test the published loopback port with:

```sh
curl -H 'Host: example.reallyme.net' http://127.0.0.1:8080/hello
```

Set your ingress hostnames and proxy ranges in
[the container config](example-server.container.jsonc) before deployment. Choose
operational route exposure in the host composition. The example `docker run`
command publishes only on loopback.
