/**
 * The eight Rust remote stores, for the Node binding.
 *
 * Each store is a class with the same two methods — an async `fetch()` and
 * a `describe()` — which is exactly the shape the base package's
 * `setRemote` takes. `useStore` is the bridge between the two: a fetch is
 * a network round trip and must not sit on the event loop, and the remote
 * layer is filled from a worker thread and must be handed a synchronous
 * answer.
 */

import type { DynamicConfig, Document } from "dynamic-config-node";

/** What every call answers: a value, or the failure with its kind. */
export interface Outcome<T> {
  readonly ok: boolean;
  readonly value?: T;
  readonly error?: {
    readonly kind: string;
    readonly path: string;
    readonly originKind: string;
    readonly origin: string | null;
    readonly message: string;
  };
}

/** What every store here answers. */
export interface Store {
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** Consul's key/value store. */
export class Consul implements Store {
  constructor(
    address: string,
    key?: string | null,
    keys?: string[] | null,
    prefix?: string | null,
    format?: "json" | "toml" | "yaml" | null,
    token?: string | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** HashiCorp Vault's KV v2 store. A secret is JSON, so there is no format. */
export class Vault implements Store {
  constructor(
    address: string,
    mount: string,
    path?: string | null,
    paths?: string[] | null,
    format?: "json" | "toml" | "yaml" | null,
    token?: string | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** A Redis key, or a named list of them. The credential rides in the URL. */
export class Redis implements Store {
  constructor(
    url: string,
    key?: string | null,
    keys?: string[] | null,
    prefix?: string | null,
    format?: "json" | "toml" | "yaml" | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** An etcd v3 key/value store. */
export class Etcd implements Store {
  constructor(
    endpoints: string[],
    key?: string | null,
    keys?: string[] | null,
    prefix?: string | null,
    format?: "json" | "toml" | "yaml" | null,
    username?: string | null,
    password?: string | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** A NATS JetStream key/value bucket. */
export class Nats implements Store {
  constructor(
    server: string,
    bucket: string,
    key?: string | null,
    keys?: string[] | null,
    format?: "json" | "toml" | "yaml" | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** An object in S3, or in anything that speaks its API. */
export class S3 implements Store {
  constructor(
    bucket: string,
    key?: string | null,
    keys?: string[] | null,
    prefix?: string | null,
    format?: "json" | "toml" | "yaml" | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** A Google Cloud Firestore document. */
export class Firestore implements Store {
  constructor(
    project: string,
    path?: string | null,
    paths?: string[] | null,
    accessToken?: string | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** A file in a git repository, at a branch, a tag or a commit. */
export class Git implements Store {
  constructor(
    url: string,
    path?: string | null,
    paths?: string[] | null,
    prefix?: string | null,
    branch?: string | null,
    tag?: string | null,
    commit?: string | null,
    format?: "json" | "toml" | "yaml" | null,
    token?: string | null,
    timeoutMs?: number | null,
  );
  fetch(): Promise<Outcome<Document>>;
  describe(): string;
}

/** What a later refresh needs. */
export interface Installed {
  /** Fetch again, fill the remote layer, and reload. */
  refresh(): Promise<unknown>;
}

/**
 * Installs `store` into `config`, fetches once, and reloads.
 *
 * The bridge between an async fetch and the synchronous source the engine's
 * remote layer is filled from: the last answer is kept, and every refresh
 * replaces it before the layer is filled.
 */
export function useStore<T>(config: DynamicConfig<T>, store: Store): Promise<Installed>;

/** One fetch, unwrapped into a document or a `DynamicConfigError`. */
export function fetchFrom(store: Store): Promise<Document>;

export function packageVersion(): string;
export function engineVersion(): string;
