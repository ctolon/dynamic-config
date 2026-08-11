# Roadmap

What is not in the crate yet and might be. Everything that shipped is described
in [README.md](README.md); what will *not* be built, and why, is under
[Not planned](book/src/limitations.md#not-planned) there.

Tags: **[viper]** exists in Go's [Viper](https://github.com/spf13/viper) and does
not here. **[figment]** is something the underlying loader,
[figment](https://docs.rs/figment), can do that this crate does not expose.
**[own]** is neither — an idea from using the thing.

<!-- Keep this shape. An item earns a place here when it has a *decision* in it:
     what the alternatives are, and why one of them is not obviously right. An
     item that is only "add X" belongs in an issue. -->

---

## The next two releases

Decided, not aspirational — each item below keeps its full description in
its own section; this is only who ships when. The shape follows
[the 1.0 doctrine](#the-road-to-10-is-stabilisation-not-features-own): 0.3
stabilises what exists, 0.4 adds the one engine piece everything after it
needs, and nothing jumps the queue without displacing something.

**0.3 — stabilisation.** No new surface except where an instruction
becomes a guarantee:

- Generation-fenced remote pushes — the last unfenced door, and breaking
  it now is cheaper than ever again.
- loom/shuttle model checking over `ConfigCell`, `Remote`, `Changes`, the
  watch registry and group reload — the concurrency claims get proven
  before the API freezes around them.
- Tidy the module tree (`builder.rs`, `watch.rs`, `redirects.rs` earn the
  loader's treatment).
- The figment abstraction-leak review from the 1.0 doctrine — the review
  half happens here; whatever it decides lands later.
- Container suites that fail on behaviour, not scheduling luck.
- Security-tab triage: the first full pass, and the standing rule.
- Release and branch mechanics, polished — including the root-changelog
  rotation that has now been forgotten by hand twice.
- Writing a store, promoted into the book.

**0.4 — the instance engine, and the evidence:**

- `Dynamic<T>` + per-instance watch identity + value export — phase one
  of [python-bindings-plan.md](python-bindings-plan.md), shipped as Rust
  API on its own merits.
- Benchmarks that would convince a stranger.
- A bundle for single-generation groups.
- `dynamic-config-cli` graduates to crates.io, redacted-by-default.
- Encrypting the last-known-good cache.
- Coverage threshold joins the gates.

**0.5 and later, pulled by demand:** the Python bindings proper (phases
two through five of the plan), `proc_macro_crate` rename, key aliases
across sections, `WriteDurability`, the embedded no-alloc wait queue, the
shared auth core and its dependents (`with_timeout` symmetry,
`ErrorKind::Auth`), runtime-agnostic S3 sleep, multi-key remote
documents, an eighth store when somebody asks, serde_yaml's future as
upstream decides it, and — far out, designed in the open first — the
config server.

## Layers

### Key aliases across sections **[viper]**
`alias("pool.size", "pool.max_size")` moves a path within one section. A value
that moved *between* sections — `server.timeout` becoming `http.timeout` — is
not expressible, because a `LoadSpec` resolves one section at a time.

Doable by resolving the other section during the alias pass. Unclaimed, and
worth a real case first: the cost is that a load then depends on a section the
type does not own.

---

## Writing

### Encrypting the last-known-good cache **[own]**
`save_encrypted` covers what a program deliberately writes. The
[cache](book/src/persistence.md#last-known-good) still writes plaintext, and its three modes
exist precisely because that is a trade-off.

A fourth mode — encrypted, full fidelity — would collapse the trade-off: it
would recover completely without leaving secrets readable. What stops it today
is that the cache is written on a path with no obvious place to put a recipient
list, and inventing a process-wide one would undo the reason `save_encrypted`
takes it at the call site.

---

## Remote stores

### Reading several keys as one document **[own]**
Every store crate reads one key. A deployment that splits configuration across a
prefix — `myapp/db`, `myapp/server` — installs one source per section, which
works and is a little tedious.

Merging a prefix into one document is easy for etcd, Consul and Redis, awkward
for Vault and Firestore, and needs an ordering between keys defined before it
means anything. Possible, unclaimed.

### A store nobody has asked for yet **[own]**
`RemoteSource` and `AsyncRemoteSource` are public, so a new store is a crate
rather than a patch to this one. Seven exist; an eighth is worth adding when
somebody wants it, not before.

Each is a client dependency, a container in CI, an authentication story and a
set of failure modes to get right — the seven that exist took that seriously,
and an eighth done casually would be worse than none.

---

## After 0.1.0

### Generation-fenced remote pushes **[own]**

`refresh_remote` is fenced: a fetch that started against a source that has
since been replaced is discarded by its generation token. The *push* path
is not — `Remote::install(document)` takes no token, and the doc's answer
is "stop the old watch loop first", which is an instruction where an API
could be a guarantee. A watch handle that carries the generation it was
started under (an install sink handed to the loop, say) would make a stale
watcher's push impossible rather than discouraged. The most valuable
remaining fence in the remote design.

### A bundle for single-generation groups **[own]**

`ReloadGroup` promises all-or-nothing *installation* and says honestly that
the commits are separate swaps: a reader can still observe member A's new
generation next to member B's old one for an instant. For the few cases
where even that instant matters — a certificate and its port — the clean
answer is structural: one type holding both sections, one `ArcSwap`, one
generation. Worth offering as a documented pattern (or a small `Bundle`
helper) rather than leaving each caller to rediscover it.

### Benchmarks that would convince a stranger **[own]**

The read-path numbers come from a hand-rolled loop in this repository —
honest about what it is, but not evidence a skeptic can use. The upgrade:
criterion (and iai-callgrind for instruction counts), a concurrent
readers-during-reload scenario, reload latency, large-config scaling
(hundreds to hundreds of thousands of keys), and an allocation profile.
Cross-library comparisons stay out of CI and in a written-up experiment —
they rot too fast to gate on.

### Container suites that shrug off a slow daemon **[own]**

The store tests boot real servers, and a shared CI runner sometimes takes
longer to start one than the wait allows — `WaitContainer(StartupTimeout)`
from a Vault that was going to be fine in ten more seconds. That is a
false positive: the code did not change, the daemon was slow, and the fix
today is "re-run the job". Worth building in instead: one startup retry
with a fresh container before declaring failure, startup windows measured
from what CI actually exhibits rather than guessed (Vault already went to
120 s once), and — since the failure mode is concurrency-driven — keeping
the `--test-threads=2` cap honest as suites grow. The gate should fail on
behaviour, not on scheduling luck.

### `dynamic-config-cli` on crates.io **[own]**

It ships in-repo, deliberately unpublished: crates.io versions are
permanent, and an Experimental surface should settle before it claims a
name. Next release it graduates: drop `publish = false`, give the crate its
own README and the symlinked LICENSE the packaging check demands, add it to
`release.yml`'s third wave and the dry-run's README/LICENSE list, and put
`cargo install dynamic-config-cli` in the book. Shell completions and a man
page ride along — clap generates both for one line each. Before it claims
the name, `explain` flips to redacted-by-default with `--show-values` to
opt in: an Experimental tool may ask the user to know which paths are
sensitive, a published one should not.

### Triage the Security tab, then keep it triaged **[own]**

GitHub's security surface (Dependabot alerts, code scanning, the
dependency-review scorecard warnings) accumulates findings that are each
either *fixable now*, *waiting on an upstream release*, or *consciously
accepted*. Right now the distinction lives in nobody's head. One pass over
everything open, each finding landing in one of those three buckets — the
accepted ones with the reason written into `deny.toml` or `SECURITY.md`, so
the next person sees a decision instead of a backlog. Then the standing
rule: an alert is triaged within a release cycle or it blocks one.

### Tidy the module tree **[own]**

Folders and files, not crates — the crate split stays refused in
[Not planned](book/src/limitations.md#not-planned). Within the core, some
modules earned directories (`loader/`, `expand/`) and others outgrew single
files without getting one: `builder.rs` now carries the builder, its watch
plumbing and the `Configured` slot in one file, `watch.rs` mixes the
debounce machinery with the registry and the `Watched` description, and
`redirects.rs` holds every remaining facade macro. One pass that gives each
grown module the same treatment the loader got — a directory, one concern
per file, the module doc naming what lives where — so the onboarding tour's
"module by module" chapter keeps mapping one heading to one place.

### A config server **[own]**

The other half of the distribution story, in the spirit of Spring Cloud Config Server:
a small service that owns the files (or fronts a store), serves resolved
sections over HTTP, and pushes changes to subscribed clients — so a fleet
of services shares one source of truth without each carrying store
credentials. The client side is already here (`RemoteSource` + a watch
loop); the server would be a new crate with its own threat model (authn,
who may read which section, audit). Far future, and worth designing in the
open before building.

### Writing a store, promoted into the book **[own]**

The book shows `impl RemoteSource` for a toy; the real how-to — the watch
loop and its `Watching` token, credential refresh with one retry, the
shared contract table's obligations, what the container tests must pin —
lives only in an internal contributor skill. Promote it to a chapter, so a
third-party store crate can be written without reading seven existing ones
for the pattern.

### Python bindings: Rust resolves, Pydantic validates **[own]**

A PyO3 extension pairing this runtime with Pydantic: Rust owns sources,
layering, watching, recovery and provenance; Pydantic owns the schema and
its validators; Python reads a cached model for the price of an attribute
lookup, re-validated once per reload, never per read. Needs two core
changes that stand on their own — an instance engine (`Dynamic<T>`, for
every Rust user who wanted two configurations of one type) and a watch
identity beyond `TypeId`. The full design — decorator and class APIs, the
GIL strategy, secrets derived from `SecretStr` fields rather than
re-declared, the zero-bug test battery, wheels — is written up in
[python-bindings-plan.md](python-bindings-plan.md), which is the
reference; this entry only tracks that it happens.

### The road to 1.0 is stabilisation, not features **[own]**

Two releases in two days is a build phase, not a track record, and the
API surface is now wide enough that its cost compounds. Before 1.0: a
deliberate quiet period — 0.3 as an API-review release (including a design
pass on where figment's abstractions leak into this crate's contract:
top-level tables, profile consumption, prefix-grained env provenance),
then real external users on 0.4+, then a freeze candidate. New capability
proposals queue behind stability during that window. The problem worth
solving by then is not a missing feature; it is that nothing this
sophisticated has been beaten up by strangers yet.

### Release and branch mechanics, polished **[own]**

A collection of paper cuts from cutting two releases:

- **`main` becomes the default branch.** Mention-triggered workflows run
  from the default branch's workflow file, and "the branch visitors land
  on" should be the released one. Requires re-pointing the paths-filtered
  workflows and the branch-protection assumptions ci.yml documents.
- **Squash merges for `dev` to `main`.** The promotion PR carries dozens of
  commits whose story the changelog already tells; one commit per release
  on `main` reads better and makes `git log main` the release history.
- **The promotion PR titles itself.** `propose.sh`/`promote.sh` should name
  the version when the push carries a bump — "release 0.2.0", not
  "promote dev to main" — so the PR list is scannable.
- **The root changelog rotates itself.** `cargo release` rotates the
  package changelogs; the workspace one is hand-rotated every release and
  was forgotten once already. A pre-release hook can do it.


### A real no-alloc wait queue for the embedded crate **[own]**
`ConfigCell<T, const WAITERS>` sizes the parking lot, but N > WAITERS still
degrades to wake-churn (documented). An intrusive list would fix it without
an allocator; it also drags `unsafe` into a crate that forbids it. That
trade deserves its own design pass.

### `WriteDurability` as API **[own]**
0.1.0 fsyncs every atomic write, unconditionally. If someone measures real
pain from that, a `Normal`/`Fsync` mode is the escape hatch — not before.

### loom / shuttle model checking **[own]**
The barrier tests pin the known interleavings; loom explores the unknown
ones. `Remote`'s state machine is the natural first target, but loom wants
its own sync-type shims — an investment, not an afternoon.

### `proc_macro_crate` rename support **[own]**
`::dynamic_config` is hardcoded in the expansion, so renaming the dependency
breaks. `proc-macro-crate` fixes it at the cost of a parsing dependency in
the macro crate.

### serde_yaml's future **[own]**
Archived upstream (`0.9.34+deprecated`); figment pulls it regardless, so a
local switch buys nothing. Track figment; move when it moves.

### A shared auth core for the HTTP stores **[own]**
Consul, Vault and Firestore now share the margin (`REFRESH_WITHIN`) but
still triplicate the Session/Token machinery. Reconcile the semantics on
paper first; extract second.

### `with_timeout` symmetry across stores **[own]**
The three ureq crates take `with_timeout`; etcd/NATS/S3 configure timeouts
through their clients' own vocabulary. Either add pass-throughs or document
the asymmetry per README — decide once someone actually trips on it.

### Runtime-agnostic S3 watch sleep **[own]**
Blocked on the AWS SDK itself being tokio-bound; revisit if smithy's
runtime abstraction ever makes executor-independence real.

### `ErrorKind::Auth` **[own]**
The stores classify 401/403 internally now; a public variant would let a
caller treat "credentials are wrong" as a program-visible state. Decide the
boundary with a real consumer in hand.

### Coverage threshold + release gates **[own]**
Coverage reports exist (CI artifact + summary); a threshold waits until the
number stabilises post-0.1.0.

