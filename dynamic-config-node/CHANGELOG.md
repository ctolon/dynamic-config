# Changelog

All notable changes to the `dynamic-config` npm package are documented
here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

This package versions independently of the Rust crates: it *embeds* the
engine rather than depending on a published version of it, so a Rust-only
release has nothing in it for a Node user.

<!-- Add entries under `Unreleased` as you go, and move the whole block
     under a new version heading at release time. -->

## [Unreleased]

## 0.0.1

The first release: the engine, the schema door, the watcher and the whole
diagnostic surface, through Node-API.

### Added

- **`DynamicConfig<T>`**, generic over whatever the validator returns —
  files, discovery, environment, `.env`, a secrets directory, profiles,
  the runtime layers, and the last-known-good cache.
- **A validator is a function.** Zod's `parse`, an Ajv validator through
  `ajvValidator`, a function of your own, or nothing at all for a
  configuration read by dotted path. No schema library is a dependency of
  this package.
- **A file watcher**, with the property the whole design is for: a
  document the schema refuses installs nothing and leaves the previous one
  serving, from the watcher thread exactly as from an explicit reload.
- **Remote stores written in JavaScript**: `setRemote(fetch, described)`,
  `refreshRemote()` and `remoteStatus()`. The eight Rust stores are a
  second package, for the reason they are a second wheel in Python.
- **The diagnostics**: `sourceOf`, `isSet`, `explain`, `check`,
  `snapshot`, `status`, and `DynamicConfigError` with `kind`, `path`,
  `originKind` and `origin` on it.
- Hand-written TypeScript definitions, so `current()` is `T` under
  `strict: true` with nothing cast.

### Notes

- **There is no `initSync`.** Validation happens inside the load, so the
  load runs on a worker thread and calls back into the event loop; a
  synchronous `init()` would be the loop waiting for itself. The README
  says so rather than leaving somebody to find it at startup.
