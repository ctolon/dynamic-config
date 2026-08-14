# dynamic-config (Node.js)

Hot-reloadable configuration for Node.js: **Rust resolves, your schema
validates, JavaScript reads a cached object.**

```sh
npm install dynamic-config
```

```ts
import { DynamicConfig, zodValidator } from "dynamic-config"
import { z } from "zod"

const Database = z.object({ host: z.string(), port: z.number().default(5432) })

const config = new DynamicConfig({ key: "db", validate: zodValidator(Database) })

const db = await config.file("config.toml").env("APP_").initAndCurrent()
//    ^? { host: string; port: number }
```

The engine is the [`dynamic-config`] Rust crate — files, environment
layering, `.env`, profiles, discovery, precedence, a debounced file
watcher, last-known-good recovery and provenance. The schema is whatever
your program already uses: **a validator here is a function**, so Zod's
`parse` is one, an Ajv validator becomes one in four lines, and a plain
function of your own is one too. Nothing is a dependency of this package
except the engine, which is compiled into it.

**Validation runs once per successful resolve, never per read.**
`current()` is a property read on a cached object, so reading configuration
on every request costs nothing.

## What it gives you

```ts
await config.init()                    // load, validate, install
await config.reload()                  // again, on demand
config.watch({ debounceMs: 250 })      // and again on every file change
config.current()                       // the document in force
config.get("pool.maxSize", 8)          // …or one value, by dotted path

config.onReload((document) => …)       // every install
config.onChange("pool.maxSize", …)     // one path, when it moves

config.sourceOf("port")                // which layer wins, and from where
config.isSet("port")
config.explain("port")                 // every layer's answer, as a table
config.check()                         // would it load? any unknown keys?
config.snapshot()                      // the document, and how old it is
config.status()                        // for a health endpoint
```

**A rejected document changes nothing.** A file edited into something the
schema refuses leaves the previous document serving and reports the
failure; that is the property the whole design is for, and it holds for a
watcher-driven reload exactly as it does for an explicit one.

## Errors carry what a program branches on

```ts
try {
  await config.init()
} catch (failure) {
  if (failure instanceof DynamicConfigError) {
    failure.kind        // "io" | "parse" | "missing" | "invalid" | "remote" | …
    failure.path        // the dotted key path
    failure.originKind  // "file" | "env" | "remote" | …
    failure.origin      // the file, the variable, the store
  }
}
```

The same words the Rust `ErrorKind` and the Python exception hierarchy
use, so the same condition is called the same thing in all three.

## Remote stores

A store is a function that answers `{ text, format }`:

```ts
config.setRemote(() => latest, "our config service")
await config.refreshRemote()   // fill the remote layer
await config.reload()          // resolve and validate it
config.remoteStatus()          // reachable, fetches, failures
```

It must be **synchronous** — it is called from a worker thread through the
event loop, and a promise cannot be awaited from there. An async source
keeps its own last answer:

```ts
let latest = { text: "{}", format: "json" as const }
setInterval(async () => { latest = await read() }, 30_000)
config.setRemote(() => latest, "our config service")
```

The eight Rust stores — etcd, Consul, Vault, NATS, Redis, S3, Firestore,
git — are **not in this package**, for the reason they are a second wheel
in Python: a gRPC stack and an AWS SDK in every `npm install` is not a
default. They arrive as `@dynamic-config/remote`.

## Two things this binding deliberately does not have

**`initSync`.** Every load is asynchronous, and that is not a style
choice: validation happens *inside* the load, before anything installs,
which means the load runs on a worker thread and calls back into the event
loop. A synchronous `init()` would be the loop waiting for itself — a
deadlock at startup, which is the worst place to put one. Use top-level
`await`, or `await config.init()` in your `main`.

**A configuration engine in the browser.** There is no filesystem, no
watcher and no store there. What ships to a client is a snapshot somebody
chose to send it, which is a serialisation decision rather than a
configuration one.

## Versions

Two numbers, because they move on two schedules: `packageVersion()` is
this package's, and `engineVersion()` is the Rust crate it was built
against.

## The book

[**dynamic-config for Node.js**](https://ctolon.github.io/dynamic-config/node/)
— the API reference, schemas, watching, the frameworks, the remote stores,
what crosses the boundary and what this binding will not do.

The engine's own behaviour — precedence, profiles, discovery, the
last-known-good cache, the document-shape rules — is the
[Rust book](https://ctolon.github.io/dynamic-config/), because it is the
same engine.

## Examples

Twelve, in [`examples/`](examples/): a quick start, layering, watching,
diagnostics, Express, Fastify, Zod/Ajv/no-schema side by side, a NestJS
provider, Next.js server components, and the React one that draws the
browser boundary rather than pretending there is none. The runnable ones
run in CI on every Node version this package claims; the TypeScript ones
are typechecked there.

## Supported platforms

| | x64 | arm64 |
|---|---|---|
| Linux (glibc) | ✅ | ✅ |
| macOS | ✅ | ✅ |
| Windows | ✅ | — |

One prebuilt binary per platform, installed as an optional dependency — so
`npm install` downloads one, not five, and compiles nothing.

**Node 18, 20, 22 and 24** are tested in CI. Node-API is ABI-stable, so
the same binary serves versions released after this one; the matrix exists
because the JavaScript half is ordinary code that a version can break.

[`dynamic-config`]: https://crates.io/crates/dynamic-config
