# Changelog

All notable changes to `dynamic-config-node-remote` are documented here. The
format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

This package versions **with** `dynamic-config`: it hands documents to the
base package, and a gap between them is a pair nobody built.

## [Unreleased]

## 0.0.1

### Added

- **The eight Rust stores** — etcd, Consul, Vault, NATS, Redis, S3,
  Firestore and git — each a class with an async `fetch()` and a
  `describe()`, which is the shape the base package's `setRemote` takes.
- **`useStore(config, store)`**, the bridge between an async fetch and the
  synchronous source the engine's remote layer is filled from: the last
  answer is kept, and the handle it returns refreshes it.
- Descriptions that never carry a credential, by the store crates' own
  redaction rather than by a second copy of it.
