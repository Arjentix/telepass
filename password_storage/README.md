# Telepass Password Storage

Storage for all user passwords.

Information about supported `gRPC` methods see in [proto-file](../proto/password_storage.proto).

Uses `PostgreSQL` database to store records.

## Running with Docker

All commands are shown for `password_storage` directory.

### Docker Compose

#### Production

```
docker compose up
```

#### Development

```bash
docker compose -f docker-compose.yml -f docker-compose.dev.yml up
```

### Standalone Dockerfile (without Database)

#### Production

```bash
docker build .. -f Dockerfile -t telepass/password_storage
```

#### Development

```bash
docker build .. -f Dockerfile --target dev-runtime -t telepass/password_storage:dev
```

## Local build

All commands are shown for `password_storage` directory.

**Note:** `executable` feature is required to the build binary. Having this feature allows to not to force library users have redundant dependencies.

### Production

```bash
cargo build --release --features executable
```

### Development

```bash
cargo build --no-default-features --features "executable, development"
```

## Notes about *development* builds

Development configuration disables `tls` certificate checking and enables `gRPC` reflection for easier testing.

For more details about features see [`Cargo.toml`](Cargo.toml).
