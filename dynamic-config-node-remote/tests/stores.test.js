"use strict";

/**
 * The eight stores, without a server.
 *
 * What can be asserted with no container is the half that is this
 * package's own: the constructors, the key shapes, the descriptions that
 * must not carry a credential, and the failure a store that is not there
 * produces. The other half — a document actually arriving — is the store
 * crates' container suites, which already run against real servers.
 */

const test = require("node:test");
const assert = require("node:assert/strict");

const stores = require("../js/index.js");
const { DynamicConfigError } = require("dynamic-config-node");

/** Every store, constructed the smallest way each one allows. */
function each() {
  return [
    ["Consul", new stores.Consul("http://127.0.0.1:9", "myapp/db.json")],
    ["Vault", new stores.Vault("http://127.0.0.1:9", "secret", "myapp/db")],
    ["Redis", new stores.Redis("redis://127.0.0.1:9", "myapp/db.json")],
    ["Etcd", new stores.Etcd(["http://127.0.0.1:9"], "myapp/db.json")],
    ["Nats", new stores.Nats("nats://127.0.0.1:9", "config", "db.json")],
    ["S3", new stores.S3("myapp-config", "prod/db.json")],
    ["Firestore", new stores.Firestore("myapp", "config/db")],
    ["Git", new stores.Git("https://example.invalid/repo.git", "config/db.json")],
  ];
}

test("every store describes itself", () => {
  for (const [name, store] of each()) {
    const described = store.describe();

    assert.equal(typeof described, "string", name);
    assert.ok(described.length > 0, name);
  }
});

test("a description never carries a credential", () => {
  const redis = new stores.Redis("redis://app:hunter2@127.0.0.1:6379", "myapp/db.json");
  const git = new stores.Git("https://app:hunter2@example.invalid/repo.git", "config/db.json");

  assert.ok(!redis.describe().includes("hunter2"), redis.describe());
  assert.ok(!git.describe().includes("hunter2"), git.describe());
});

test("exactly one of key, keys and prefix", () => {
  assert.throws(
    () => new stores.Consul("http://127.0.0.1:9", "one", ["and", "several"]),
    /exactly one of the three/,
  );
  assert.throws(() => new stores.Consul("http://127.0.0.1:9"), /exactly one of the three/);
});

test("a format that is not one is refused by name", () => {
  assert.throws(
    () => new stores.Consul("http://127.0.0.1:9", "myapp/db.json", null, null, "xml"),
    /is not a format/,
  );
});

test("a git source reads one reference, not two", () => {
  assert.throws(
    () =>
      new stores.Git(
        "https://example.invalid/repo.git",
        "config/db.json",
        null,
        null,
        "main",
        "v1.0",
      ),
    /not two of them/,
  );
});

test("a store that is not there fails as `remote`, and says so", async () => {
  // Port 9 is discard: nothing listens, on any machine. The three async
  // clients get their own runtime per fetch, so this also proves that the
  // runtime is built, used and dropped without leaving the process up.
  for (const [name, store] of each()) {
    if (name === "S3" || name === "Firestore" || name === "Git") {
      // These three reach a real endpoint by name rather than a port, so a
      // failure here would be about DNS or about somebody's credentials
      // file — not about this package.
      continue;
    }

    const outcome = await store.fetch();

    assert.equal(outcome.ok, false, name);
    assert.ok(
      ["remote", "auth", "io", "backend"].includes(outcome.error.kind),
      `${name}: ${outcome.error.kind}`,
    );
    assert.ok(outcome.error.message.length > 0, name);
  }
});

test("fetchFrom throws the base package's error", async () => {
  const store = new stores.Consul("http://127.0.0.1:9", "myapp/db.json");

  await assert.rejects(() => stores.fetchFrom(store), (failure) => {
    assert.ok(failure instanceof DynamicConfigError);
    assert.equal(failure.kind, "remote");

    return true;
  });
});

test("useStore installs a document and keeps it current", async () => {
  const { DynamicConfig } = require("dynamic-config-node");

  // A store of our own with the same two methods the eight have: that is
  // the whole contract, and it is what makes `useStore` testable without a
  // server.
  let served = '{"db":{"host":"first"}}';

  const store = {
    fetch: async () => ({ ok: true, value: { text: served, format: "json" } }),
    describe: () => "a store standing in for one of the eight",
  };

  const config = new DynamicConfig({ key: "db" });
  const handle = await stores.useStore(config, store);

  assert.equal(config.current().host, "first");
  assert.equal(config.remoteDescription, "a store standing in for one of the eight");

  served = '{"db":{"host":"second"}}';
  await handle.refresh();

  assert.equal(config.current().host, "second");
  assert.equal(config.remoteStatus().fetches, 2);
});

test("both versions are reported", () => {
  assert.match(stores.packageVersion(), /^\d+\.\d+\.\d+$/);
  assert.match(stores.engineVersion(), /^\d+\.\d+\.\d+$/);
});
