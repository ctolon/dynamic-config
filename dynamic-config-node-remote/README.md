# dynamic-config-node-remote

The eight Rust remote stores for [`dynamic-config`](../dynamic-config-node)
— **etcd, Consul, Vault, NATS, Redis, S3, Firestore and git** — as a
second package.

```sh
npm install dynamic-config-node dynamic-config-node-remote
```

```ts
import { DynamicConfig } from "dynamic-config-node"
import { Etcd, useStore } from "dynamic-config-node-remote"

const config = new DynamicConfig({ key: "db", validate })
await config.file("config.toml").init()

const store = new Etcd(["http://etcd:2379"], "myapp/db.json")
const installed = await useStore(config, store)

// Later — on a timer, a signal, a webhook:
await installed.refresh()
```

## Why a second package

A gRPC stack, an AWS SDK and three HTTP clients in every `npm install
dynamic-config` is not a default anybody asked for. The same reason they
are a second wheel in Python: the engine is small, the clients are not,
and a program that reads a file should not pay for eight of them.

## What a store is

One class per store, each with the same two methods:

```ts
await store.fetch()   // { ok: true, value: { text, format } }
store.describe()      // how it names itself in an error
```

That is exactly the shape the base package's `setRemote` takes, so a store
from here is indistinguishable from one somebody wrote in JavaScript — and
`useStore` is the four lines that bridge the two: **`fetch()` is async**
because a network round trip must not sit on the event loop, and the
engine's remote layer is filled from a worker thread and must be handed a
synchronous answer, so the last one is kept.

## The stores

| Store | Constructed with |
|---|---|
| `Consul` | `address`, one of `key`/`keys`/`prefix`, `format?`, `token?`, `timeoutMs?` |
| `Vault` | `address`, `mount`, `path`/`paths`, `token?`, `timeoutMs?` |
| `Redis` | `url` (the credential rides in it), `key`/`keys`/`prefix`, `format?`, `timeoutMs?` |
| `Etcd` | `endpoints[]`, `key`/`keys`/`prefix`, `format?`, `username?`, `password?`, `timeoutMs?` |
| `Nats` | `server`, `bucket`, `key`/`keys`, `format?`, `timeoutMs?` |
| `S3` | `bucket`, `key`/`keys`/`prefix`, `format?`, `timeoutMs?` — credentials from the environment |
| `Firestore` | `project`, `path`/`paths`, `accessToken?`, `timeoutMs?` |
| `Git` | `url`, `path`/`paths`/`prefix`, one of `branch`/`tag`/`commit`, `format?`, `token?`, `timeoutMs?` |

**A description never carries a credential.** A Redis URL with a password
in it, a git URL with a token: both are redacted by the store crates' own
rule, so an error message and a log line are safe to keep.

## What is not here yet

- **Callable credentials that rotate.** The Python wheel takes a function
  that mints a fresh token; here a token is a string. A deployment whose
  credential rotates should rebuild the store — which is one line, and
  what `useStore`'s handle makes easy.
- **TLS material from bytes.** The store crates take paths and PEM; this
  package exposes neither yet, so a private authority is the platform
  trust store's business for now.
- **Watch loops.** A store's watch is a long-lived thread pushing into a
  sink. `installed.refresh()` on a timer is the shape here, and it is the
  same one `refresh_remote()` is in Rust.

Each of those is a decision to make deliberately rather than a gap to fill
quietly; [the book](https://ctolon.github.io/dynamic-config/node/remote-stores.html)
says the same thing where a reader will meet it.
