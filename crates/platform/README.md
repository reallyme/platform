# reallyme-platform

The facade re-exports app-kit and selected runtime and infrastructure kits.
Its default feature set is empty.

```sh
cargo add reallyme-platform
cargo add reallyme-platform --features native-server
cargo add reallyme-platform --features postgres,nats
```

Use `server-connect` and `server-websocket` for those native transports. S3 client
features are `s3-native-client` and `s3-worker-client`; `s3` alone exposes contracts.
See the [Platform repository](https://github.com/reallyme/platform) for examples.
