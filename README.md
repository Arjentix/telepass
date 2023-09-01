# Telepass

Self-hosted password manager for *Telegram* fully written in **Rust** 🚀🚀🚀!

This implementation accepts only one user. The idea is that all this data is very sensitive and should be hosted by users.

## Micro-services

Non-exhaustive list, because it's currently on active development

- [`Password Storage`](password_storage/README.md) – Storage for all user passwords
- [`Telegram Gate`](telegram_gate/README.md) – Node to interact with `Telegram` using bots API

## Deployment

### TLS certificates configuration

#### Quick start

```bash
./scripts/gen_certs.sh
```

#### Manual configuration

If you're familiar with TLS and how to create your own certificates then the required files are:

```
certs
├── password_storage.crt
├── password_storage.key
├── root_ca.crt
├── telegram_gate.crt
└── telegram_gate.key
```

Use `scripts/gen_certs.sh` as a reference if you have any problems.

To be continued…
