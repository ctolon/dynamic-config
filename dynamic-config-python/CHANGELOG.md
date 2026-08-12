# Changelog

All notable changes to `dynamic-config-py` are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Before 1.0, a breaking change bumps the **minor** version and anything else
bumps the patch. A change to the minimum supported Python version is
breaking.

<!-- Keep this template. Add entries under `Unreleased` as you go, and move
     the whole block under a new version heading at release time.
     (Spelled `_Unreleased_` here so cargo-release's `exactly = 1` search
     for the real heading matches only the real heading.)

## [_Unreleased_]

### Added
### Changed
### Deprecated
### Removed
### Fixed
### Security

-->

## [Unreleased]

### Added

- **Pydantic is optional, and a `dataclasses.dataclass` is a schema.**
  The base install has no dependencies at all — the engine is compiled
  into the wheel, and the stdlib already has a way to declare a record:

      pip install dynamic-config-py                     dataclasses
      pip install dynamic-config-py[pydantic]           + models
      pip install dynamic-config-py[pydantic-settings]  + BaseSettings
      pip install dynamic-config-py[all]                all of it

  Importing the package with Pydantic uninstalled loads no Pydantic
  module, and CI asserts exactly that in a bare virtualenv. Everything
  else is the same object either way — sources, precedence, watching,
  recovery, the diagnostics, the redacted cache — and what changes is
  what validation *means*. A dataclass is validated structurally:
  required fields present, no key the class never declared, nested
  dataclasses built recursively, and each value against its declared
  type, with `bool` and `int` kept apart. It does not coerce, except
  where a type parses its own text — an `Enum` takes its member's value,
  `date`/`time`/`datetime` go through `fromisoformat`, and `UUID`,
  `Path`, `Decimal` and `IPv4Address` build from theirs. Secrets are
  declared the stdlib's way, `field(metadata={"secret": True})`, and
  drive the same redaction a `SecretStr` does.
- **`init_and_current()`**, and `init_and_current_async()`, for the two
  calls that always pair. Starting up is the one moment a program wants
  both, and writing it as two statements means naming the configuration
  twice::

      db = DynamicConfig(Database, key="db").file("app.toml").init_and_current()

- **This package versions independently of the Rust crates.** It embeds
  the engine rather than depending on a published version of it, so a
  Rust-only release is not a reason to ask every Python user to upgrade.
  `dynamic_config.__version__` is this package; `__engine_version__` is
  the `dynamic-config` crate the wheel was built against.
- The first release: a PyO3 extension pairing the `dynamic-config` engine
  with Pydantic validation.
  - `DynamicConfig(Model, key=..)` with the whole source surface —
    `file`, `discover`, `env`, `nest`, `allow_empty_env`, `strict_env`,
    `env_file`, `profile_env`, `cache` — and the whole lifecycle:
    `init`/`init_async`, `load`/`load_async`, `reload`, `current`/
    `try_current`, `replace`, `watch`, `on_reload`, `changed`, and
    `changes()` as an async iterator.
  - The runtime layers (`set_default`, `set_defaults`, `set_override`,
    `set_assignments`, `clear_*`), `alias` and `bind_env`.
  - The diagnostics: `source_of`, `is_set`, `explain`, `check`,
    `snapshot`.
  - The `@dynamic_config(...)` decorator, which attaches a configuration
    to a model class without loading at import time.
- **Validation once per resolve, never per read.** A reload Pydantic
  rejects keeps the previous model serving and writes no cache, exactly
  as a Rust `validate` refusal does.
- **Secrets derived from the model**: `SecretStr` and `SecretBytes` —
  through `Optional`, unions, containers, nested models, Pydantic
  dataclasses and `RootModel` — seed the redaction the cache, `explain`
  and the scrubbed `ValidationError` all follow. A field contributes
  **every** name a file could carry it under: each alias shape Pydantic
  accepts (`AliasChoices`, `AliasPath`, `alias`, an `alias_generator`)
  and the field name, because a secret spelled the other way is still a
  secret. Over-listing costs a key nothing supplies; under-listing put a
  password in `explain` and in the "redacted" cache on disk.
- **`pydantic-settings` is supported as a schema, and translated as a
  declaration.** A `BaseSettings` class works here as any model does, and
  `DynamicConfig.from_settings(Settings, key=..)` reads its
  `SettingsConfigDict` and rebuilds it as engine sources:
  `toml_file`/`json_file`/`yaml_file` become files, `env_file` becomes
  the dotenv layer, and `env_prefix` becomes one binding per leaf field,
  so `APP_PORT` stays `APP_PORT` rather than becoming `APP_<KEY>_PORT`
  and a deployment's existing variables keep working.
  `env_nested_delimiter` and `case_sensitive` shape those names. What has
  no engine equivalent is refused at the call rather than dropped —
  `secrets_dir`, `cli_parse_args`, an overridden
  `settings_customise_sources`. Constructing a `DynamicConfig` directly
  from a class that declares sourcing warns: being the source is a fine
  thing to choose, and believing an `env_prefix` is doing something is
  not.
- **Whatever a Pydantic model may be, it may be a schema here.**
  Inheritance to any depth, mixins, `model_config` (`extra`, `frozen`,
  `populate_by_name`, `alias_generator`, `validate_assignment`), field
  and model validators in both modes, computed fields, private
  attributes, `RootModel`, Pydantic dataclasses, generic models and
  discriminated unions — each with a test in `tests/test_pydantic.py`.
- Type stubs and a `Generic[M]` facade, so `current()` type-checks as the
  caller's model; `mypy --strict` runs over it in CI.
- An exception hierarchy mirroring `ErrorKind`, each instance carrying
  `kind`, `path` and `origin`.
- `changed_paths(previous, current)` — the audit half of a reload from
  Python: which paths moved, never what they moved to, secrets included
  in the comparison and excluded from the answer.
- The read path is a Python attribute rather than a boundary crossing:
  28 ns against a module global's 20, with the two caches' agreement
  pinned by a test on every install path.
- `reload_async`, `changed_async(timeout=…)` and `watch_async` complete
  the async surface: every blocking call has a twin that runs off the
  loop, and waiting for one reload no longer means iterating for all of
  them. Cancelling either wait is noticed within a quarter second.
  `watch_async` exists because *starting* a watcher is not free even
  though the watcher itself is a thread: it resolves directories and
  registers each with the notification backend, which measures a
  fraction of a millisecond natively and single-digit milliseconds when
  `poll_interval` makes it scan a large directory first. `Watch.stop()`
  deliberately has no twin — it drops the backend and returns without
  joining the thread or draining a debounce window.
- `set_executor(pool)` and `DynamicConfig(..., executor=pool)` choose
  which thread pool pays for the blocking half of an async call — the
  Python-side twin of the Rust crate's `set_blocking_executor`. Waits
  stay on the loop's default executor, so several `changes()` iterators
  cannot starve a small pool of the reload they are waiting for.
- **Callbacks with the ergonomics they were missing.** `on_reload` is
  usable as a decorator, because the guard it returns forwards calls to
  the hook — decorating no longer rebinds the name to something you
  cannot call — and carries the function as `.hook`. `on_change(*paths)`
  is the filter almost every reload hook opened with, written once: it
  fires only when one of those paths actually moved (a path naming a
  table covers what is under it, and the first install always counts,
  so a hook that sets something up runs at startup). The comparison is
  `changed_paths`, so a changed secret is noticed without being printed.
  [Callbacks](https://ctolon.github.io/dynamic-config/python/callbacks.html)
  is the chapter; `tests/test_callbacks.py` pins the contract — what the
  arguments mean, that a read inside a hook sees the new model, that
  hooks run in registration order on the thread that reloaded, and that
  one raising does not stop the rest.
- Seventeen runnable examples in `examples/` — including FastAPI (both
  `async def` and `def` endpoints, a lifespan-owned watcher and a test
  override), Flask, Django, a three-file asyncio service, three
  decorated services on one event loop, pydantic-settings, and a
  dataclass schema with nothing installed at all — all run in CI, and `benchmarks/read_path.py` for the numbers.
- **Ruff, as the linter and the formatter**, over the package, the suite,
  the examples and the benchmarks — including `pydocstyle`, so "every
  public definition is documented" is a rule the gate keeps rather than a
  habit. Configured at the 3.9 floor, with PEP 604 (`X | None`)
  deliberately disabled: Pydantic evaluates a model's annotations when
  the class is built, so `from __future__ import annotations` does not
  make that syntax safe for a 3.9 user, and neither the type checker nor
  vermin would catch it.
- **The package is modular**: `_config`, `_decorator`, `_diagnostics`,
  `_errors`, `_executor`, `_lifetime`, `_schema` and `_settings`, with
  `__init__.py` as the public surface and nothing else. Every import a
  user writes is unchanged; `mypy --strict` now runs over all of it
  rather than over one file.
- An integration suite (`tests/test_integration.py`) that runs whole
  scenarios rather than single calls: a service starting, watching,
  reloading and shutting down with every layer in play; a bad edit
  arriving at a running service; a restart recovering from the cache;
  four threads reading while another reloads, asserting no read ever
  sees half an install; and the shipped FastAPI, Flask and Django
  examples driven the way a test suite drives them, so an example that
  rots fails the suite.

### Security

- Pydantic's `ValidationError` echoes the offending input by default;
  across this boundary it is scrubbed to locations, messages and error
  types, and attached as `InvalidError.errors`.
- Every object's `repr` shows shape rather than values, and a watcher is
  stopped at interpreter shutdown rather than left to call into a
  finalized Python.

[Unreleased]: https://github.com/ctolon/dynamic-config/compare/v0.4.0...HEAD
