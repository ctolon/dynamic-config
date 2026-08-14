//! The eight Rust stores, for the Node binding.
//!
//! A second package for the reason they are a second wheel in Python: a
//! gRPC stack, an AWS SDK and three HTTP clients in every `npm install
//! dynamic-config` is not a default anybody asked for. `npm install
//! @dynamic-config/remote` is the opt-in.
//!
//! # What a store is here
//!
//! One class per store, each with the same two methods: an **async**
//! `fetch()` that answers `{ text, format }`, and `describe()`. That is
//! exactly the shape the base package's `setRemote` takes, so the two
//! packages meet through a documented surface rather than through each
//! other's internals — and a store from here is indistinguishable from one
//! somebody wrote in JavaScript.
//!
//! ```js
//! import { DynamicConfig } from "dynamic-config"
//! import { Etcd } from "@dynamic-config/remote"
//!
//! const store = new Etcd({ endpoints: ["http://127.0.0.1:2379"], key: "myapp/db.json" })
//!
//! await config.useStore(store)   // fetches, installs, and keeps it current
//! ```
//!
//! `fetch()` is async because a network round trip must not sit on the
//! event loop; the base package's `useStore` is what turns that into the
//! synchronous answer its `setRemote` needs, by keeping the last one.
//!
//! # What is here, and what is not
//!
//! Every store's **address, keys, format and timeout**, and the credential
//! each one takes as a string. What is deliberately not here yet:
//! callable credentials that rotate (the Python wheel's `auth=` callables),
//! TLS material from bytes, and the watch loops — a watch is a long-lived
//! thread pushing into a sink, and the base package's watcher already
//! covers files. `refreshRemote()` on a timer is the shape for now, and it
//! is one line.

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::time::Duration;

use dynamic_config::{AsyncRemoteSource, Error, Fetched, Format, RemoteSource};
use napi::bindgen_prelude::{AsyncTask, Task};
use napi::Env;
use napi_derive::napi;
use serde_json::{json, Value};

/// A JSON document, as a value Node-API can name — the same newtype the
/// base package needs, for the same reason.
pub struct Json(Value);

impl napi::bindgen_prelude::TypeName for Json {
    fn type_name() -> &'static str {
        "unknown"
    }

    fn value_type() -> napi::ValueType {
        napi::ValueType::Object
    }
}

impl napi::bindgen_prelude::ToNapiValue for Json {
    unsafe fn to_napi_value(
        env: napi::sys::napi_env,
        value: Self,
    ) -> napi::Result<napi::sys::napi_value> {
        unsafe { Value::to_napi_value(env, value.0) }
    }
}

/// The union every call answers with, exactly as the base package's does.
fn ok(value: Value) -> Value {
    json!({ "ok": true, "value": value })
}

fn failed(error: &Error) -> Value {
    json!({
        "ok": false,
        "error": {
            "kind": error.kind().as_str(),
            "path": error.path(),
            "originKind": "remote",
            "origin": Value::Null,
            "message": error.to_string(),
        }
    })
}

/// A document, as the base package's `setRemote` reads it.
fn document(fetched: &Fetched) -> Value {
    json!({
        "text": fetched.text,
        "format": match fetched.format {
            Format::Json => "json",
            Format::Toml => "toml",
            Format::Yaml => "yaml",
        },
    })
}

/// `"json"` | `"toml"` | `"yaml"`, or a refusal naming what it was.
fn format_of(name: &str) -> Result<Format, Error> {
    match name {
        "json" => Ok(Format::Json),
        "toml" => Ok(Format::Toml),
        "yaml" | "yml" => Ok(Format::Yaml),
        other => Err(Error::remote(format!(
            "`{other}` is not a format; it is \"json\", \"toml\" or \"yaml\""
        ))),
    }
}

fn seconds(timeout: Option<u32>) -> Option<Duration> {
    timeout.map(|milliseconds| Duration::from_millis(u64::from(milliseconds)))
}

/// One blocking store's fetch, on a worker thread.
///
/// `pub` because it names an `AsyncTask` in a public signature, and for no
/// other reason: nothing outside this crate can construct or drive one.
pub struct Blocking(Box<dyn RemoteSource>);

impl Task for Blocking {
    type Output = Value;
    type JsValue = Json;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok(match self.0.fetch() {
            Ok(fetched) => ok(document(&fetched)),
            Err(error) => failed(&error),
        })
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(Json(output))
    }
}

/// The same for a store whose client is async. `pub` as above.
///
/// A runtime per fetch rather than a shared one: a fetch is seconds apart
/// at most and the runtime is threads, not connections — and a shared one
/// would have to outlive an addon that Node may unload.
pub struct Asynchronous(Box<dyn Fn() -> Result<Fetched, Error> + Send>);

impl Task for Asynchronous {
    type Output = Value;
    type JsValue = Json;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok(match (self.0)() {
            Ok(fetched) => ok(document(&fetched)),
            Err(error) => failed(&error),
        })
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(Json(output))
    }
}

/// Drives an async store's fetch to completion, on the worker thread that
/// is already blocked for it.
fn drive<F>(future: F) -> Result<Fetched, Error>
where
    F: std::future::Future<Output = Result<Fetched, Error>>,
{
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .map_err(|failure| Error::remote(format!("no runtime for the fetch: {failure}")))?
        .block_on(future)
}

/// How a store was told which keys to read.
enum Shape {
    One(String),
    Several(Vec<String>),
    Prefix(String),
}

impl Shape {
    /// `{ key }`, `{ keys }` or `{ prefix }` — exactly one of the three.
    fn of(
        key: Option<String>,
        keys: Option<Vec<String>>,
        prefix: Option<String>,
    ) -> Result<Self, Error> {
        match (key, keys, prefix) {
            (Some(one), None, None) => Ok(Self::One(one)),
            (None, Some(several), None) => Ok(Self::Several(several)),
            (None, None, Some(under)) => Ok(Self::Prefix(under)),
            _ => Err(Error::remote(
                "a store reads one `key`, a list of `keys`, or everything \
                 under a `prefix` — exactly one of the three",
            )),
        }
    }
}

// ── Consul ────────────────────────────────────────────────────────────

/// Consul's key/value store.
#[napi]
pub struct Consul {
    address: String,
    shape: Option<Shape>,
    format: Option<Format>,
    token: Option<String>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl Consul {
    /// `{ address, key | keys | prefix, format?, token?, timeoutMs? }`
    #[napi(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: String,
        key: Option<String>,
        keys: Option<Vec<String>>,
        prefix: Option<String>,
        format: Option<String>,
        token: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(key, keys, prefix)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;
        let format = match format {
            Some(name) => Some(
                format_of(&name).map_err(|error| napi::Error::from_reason(error.to_string()))?,
            ),
            None => None,
        };

        Ok(Self {
            described: format!("consul {address}"),
            address,
            shape: Some(shape),
            format,
            token,
            timeout: seconds(timeout_ms),
        })
    }

    /// Reads the document. A network round trip, on a worker thread.
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> AsyncTask<Blocking> {
        let mut source = dynamic_config_consul::Consul::new(
            self.address.clone(),
            match self.shape.as_ref() {
                Some(Shape::One(key)) => dynamic_config_consul::Keys::one(key.clone()),
                Some(Shape::Several(keys)) => dynamic_config_consul::Keys::several(keys.clone()),
                Some(Shape::Prefix(prefix)) => dynamic_config_consul::Keys::prefix(prefix.clone()),
                None => dynamic_config_consul::Keys::one(String::new()),
            },
        );

        if let Some(format) = self.format {
            source = source.with_format(format);
        }
        if let Some(token) = &self.token {
            source = source.with_token(token.clone());
        }
        if let Some(timeout) = self.timeout {
            source = source.with_timeout(timeout);
        }

        AsyncTask::new(Blocking(Box::new(source)))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

// ── Vault ─────────────────────────────────────────────────────────────

/// HashiCorp Vault's KV v2 store.
#[napi]
pub struct Vault {
    address: String,
    mount: String,
    shape: Option<Shape>,
    /// Accepted and ignored: a KV v2 secret is a JSON object by
    /// construction, so there is nothing to choose. Kept as a field so a
    /// caller writing every store from one table does not have to
    /// remember the exception.
    #[allow(dead_code)]
    format: Option<Format>,
    token: Option<String>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl Vault {
    /// `{ address, mount, path | paths, format?, token?, timeoutMs? }`
    #[napi(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: String,
        mount: String,
        path: Option<String>,
        paths: Option<Vec<String>>,
        format: Option<String>,
        token: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(path, paths, None)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;
        let format = match format {
            Some(name) => Some(
                format_of(&name).map_err(|error| napi::Error::from_reason(error.to_string()))?,
            ),
            None => None,
        };

        Ok(Self {
            described: format!("vault {address} mount {mount}"),
            address,
            mount,
            shape: Some(shape),
            format,
            token,
            timeout: seconds(timeout_ms),
        })
    }

    /// Reads the secret. A network round trip, on a worker thread.
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> AsyncTask<Blocking> {
        let mut source = dynamic_config_vault::Vault::new(
            self.address.clone(),
            self.mount.clone(),
            match self.shape.as_ref() {
                Some(Shape::Several(paths)) => dynamic_config_vault::Keys::several(paths.clone()),
                Some(Shape::One(path)) => dynamic_config_vault::Keys::one(path.clone()),
                _ => dynamic_config_vault::Keys::one(String::new()),
            },
        );

        // No `with_format`: a KV v2 secret is a JSON object by
        // construction, so there is nothing to choose. `format` is
        // accepted and ignored rather than refused, because every other
        // store here takes one and a caller writing them from a table
        // should not have to remember the exception.
        if let Some(token) = &self.token {
            source = source.with_token(token.clone());
        }
        if let Some(timeout) = self.timeout {
            source = source.with_timeout(timeout);
        }

        AsyncTask::new(Blocking(Box::new(source)))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

// ── Redis ─────────────────────────────────────────────────────────────

/// A Redis key, or a named list of them.
#[napi]
pub struct Redis {
    url: String,
    shape: Option<Shape>,
    format: Option<Format>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl Redis {
    /// `{ url, key | keys | prefix, format?, timeoutMs? }`
    ///
    /// The credential rides in the URL — `redis://user:password@host` —
    /// which is where Redis puts it and where every Redis client reads it.
    #[napi(constructor)]
    pub fn new(
        url: String,
        key: Option<String>,
        keys: Option<Vec<String>>,
        prefix: Option<String>,
        format: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(key, keys, prefix)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;
        let format = match format {
            Some(name) => Some(
                format_of(&name).map_err(|error| napi::Error::from_reason(error.to_string()))?,
            ),
            None => None,
        };

        Ok(Self {
            // The URL is *not* the description: it carries a password, and
            // a description ends up in an error message. The store crate's
            // own `describe` redacts it; this names the host and stops.
            // The URL is *not* the description: it carries a password,
            // and a description ends up in an error message. This is the
            // store crates' own redaction, so the rule is one rule.
            described: format!(
                "redis {}",
                dynamic_config_store_core::redacted(
                    &url,
                    dynamic_config_store_core::LoneAuthority::Username,
                )
            ),
            url,
            shape: Some(shape),
            format,
            timeout: seconds(timeout_ms),
        })
    }

    /// Reads the key. A network round trip, on a worker thread.
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> napi::Result<AsyncTask<Blocking>> {
        let mut source = dynamic_config_redis::Redis::new(
            &self.url,
            match self.shape.as_ref() {
                Some(Shape::One(key)) => dynamic_config_redis::Keys::one(key.clone()),
                Some(Shape::Several(keys)) => dynamic_config_redis::Keys::several(keys.clone()),
                Some(Shape::Prefix(prefix)) => dynamic_config_redis::Keys::prefix(prefix.clone()),
                None => dynamic_config_redis::Keys::one(String::new()),
            },
        )
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        if let Some(format) = self.format {
            source = source.with_format(format);
        }
        if let Some(timeout) = self.timeout {
            source = source.with_timeout(timeout);
        }

        Ok(AsyncTask::new(Blocking(Box::new(source))))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

// ── etcd ──────────────────────────────────────────────────────────────

/// An etcd v3 key/value store.
#[napi]
pub struct Etcd {
    endpoints: Vec<String>,
    shape: Option<Shape>,
    format: Option<Format>,
    username: Option<String>,
    password: Option<String>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl Etcd {
    /// `{ endpoints, key | keys | prefix, format?, username?, password?, timeoutMs? }`
    #[napi(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        endpoints: Vec<String>,
        key: Option<String>,
        keys: Option<Vec<String>>,
        prefix: Option<String>,
        format: Option<String>,
        username: Option<String>,
        password: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(key, keys, prefix)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;
        let format = match format {
            Some(name) => Some(
                format_of(&name).map_err(|error| napi::Error::from_reason(error.to_string()))?,
            ),
            None => None,
        };

        Ok(Self {
            described: format!("etcd {}", endpoints.join(", ")),
            endpoints,
            shape: Some(shape),
            format,
            username,
            password,
            timeout: seconds(timeout_ms),
        })
    }

    /// Reads the document. gRPC, so the fetch drives a runtime of its own.
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> AsyncTask<Asynchronous> {
        let endpoints = self.endpoints.clone();
        let keys = match self.shape.as_ref() {
            Some(Shape::One(key)) => dynamic_config_etcd::Keys::one(key.clone()),
            Some(Shape::Several(keys)) => dynamic_config_etcd::Keys::several(keys.clone()),
            Some(Shape::Prefix(prefix)) => dynamic_config_etcd::Keys::prefix(prefix.clone()),
            None => dynamic_config_etcd::Keys::one(String::new()),
        };
        let format = self.format;
        let credential = self.username.clone().zip(self.password.clone());
        let timeout = self.timeout;

        AsyncTask::new(Asynchronous(Box::new(move || {
            let endpoints = endpoints.clone();
            let keys = keys.clone();
            let credential = credential.clone();

            drive(async move {
                let options = match credential {
                    Some((user, password)) => etcd_options_with_user(&user, &password),
                    None => dynamic_config_etcd::ConnectOptions::new(),
                };

                let mut source =
                    dynamic_config_etcd::Etcd::with_options(endpoints, keys, options).await?;

                if let Some(format) = format {
                    source = source.with_format(format);
                }
                if let Some(timeout) = timeout {
                    source = source.with_timeout(timeout);
                }

                source.fetch().await
            })
        })))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

/// etcd's own options, with a name and password on them.
fn etcd_options_with_user(user: &str, password: &str) -> dynamic_config_etcd::ConnectOptions {
    dynamic_config_etcd::ConnectOptions::new().with_user(user.to_owned(), password.to_owned())
}

// ── NATS ──────────────────────────────────────────────────────────────

/// A NATS JetStream key/value bucket.
#[napi]
pub struct Nats {
    server: String,
    bucket: String,
    shape: Option<Shape>,
    format: Option<Format>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl Nats {
    /// `{ server, bucket, key | keys, format?, timeoutMs? }`
    #[napi(constructor)]
    pub fn new(
        server: String,
        bucket: String,
        key: Option<String>,
        keys: Option<Vec<String>>,
        format: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(key, keys, None)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;
        let format = match format {
            Some(name) => Some(
                format_of(&name).map_err(|error| napi::Error::from_reason(error.to_string()))?,
            ),
            None => None,
        };

        Ok(Self {
            described: format!("nats {server} bucket {bucket}"),
            server,
            bucket,
            shape: Some(shape),
            format,
            timeout: seconds(timeout_ms),
        })
    }

    /// Reads the document. A streaming client, so the fetch drives a
    /// runtime of its own.
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> AsyncTask<Asynchronous> {
        let server = self.server.clone();
        let bucket = self.bucket.clone();
        let keys = match self.shape.as_ref() {
            Some(Shape::Several(keys)) => dynamic_config_nats::Keys::several(keys.clone()),
            Some(Shape::One(key)) => dynamic_config_nats::Keys::one(key.clone()),
            _ => dynamic_config_nats::Keys::one(String::new()),
        };
        let format = self.format;
        let timeout = self.timeout;

        AsyncTask::new(Asynchronous(Box::new(move || {
            let server = server.clone();
            let bucket = bucket.clone();
            let keys = keys.clone();

            drive(async move {
                let mut source = dynamic_config_nats::Nats::new(server, bucket, keys).await?;

                if let Some(format) = format {
                    source = source.with_format(format);
                }
                if let Some(timeout) = timeout {
                    source = source.with_timeout(timeout);
                }

                source.fetch().await
            })
        })))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

// ── S3 ────────────────────────────────────────────────────────────────

/// An object in S3, or in anything that speaks its API.
#[napi]
pub struct S3 {
    bucket: String,
    shape: Option<Shape>,
    format: Option<Format>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl S3 {
    /// `{ bucket, key | keys | prefix, format?, timeoutMs? }`
    ///
    /// Credentials, region and endpoint come from **the environment**, the
    /// way every AWS SDK reads them: `AWS_ACCESS_KEY_ID`, `AWS_REGION`,
    /// `AWS_ENDPOINT_URL` and the rest, plus the instance and container
    /// roles. A binding that took them as arguments would be a second,
    /// worse copy of a resolution chain the SDK already has and every
    /// other AWS tool on the machine already agrees with.
    #[napi(constructor)]
    pub fn new(
        bucket: String,
        key: Option<String>,
        keys: Option<Vec<String>>,
        prefix: Option<String>,
        format: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(key, keys, prefix)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;
        let format = match format {
            Some(name) => Some(
                format_of(&name).map_err(|error| napi::Error::from_reason(error.to_string()))?,
            ),
            None => None,
        };

        Ok(Self {
            described: format!("s3 bucket {bucket}"),
            bucket,
            shape: Some(shape),
            format,
            timeout: seconds(timeout_ms),
        })
    }

    /// Reads the object. The SDK is async, so the fetch drives a runtime.
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> AsyncTask<Asynchronous> {
        let bucket = self.bucket.clone();
        let keys = match self.shape.as_ref() {
            Some(Shape::One(key)) => dynamic_config_s3::Keys::one(key.clone()),
            Some(Shape::Several(keys)) => dynamic_config_s3::Keys::several(keys.clone()),
            Some(Shape::Prefix(prefix)) => dynamic_config_s3::Keys::prefix(prefix.clone()),
            None => dynamic_config_s3::Keys::one(String::new()),
        };
        let format = self.format;
        let timeout = self.timeout;

        AsyncTask::new(Asynchronous(Box::new(move || {
            let bucket = bucket.clone();
            let keys = keys.clone();

            drive(async move {
                let mut source = dynamic_config_s3::S3::new(bucket, keys).await?;

                if let Some(format) = format {
                    source = source.with_format(format);
                }
                if let Some(timeout) = timeout {
                    source = source.with_timeout(timeout);
                }

                source.fetch().await
            })
        })))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

// ── Firestore ─────────────────────────────────────────────────────────

/// A Google Cloud Firestore document.
#[napi]
pub struct Firestore {
    project: String,
    shape: Option<Shape>,
    access_token: Option<String>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl Firestore {
    /// `{ project, path | paths, accessToken?, timeoutMs? }`
    ///
    /// A Firestore document is a typed map rather than a text file, so
    /// there is no format to choose. `accessToken` is a token minted
    /// outside the process — workload identity is the other half and needs
    /// no argument at all.
    #[napi(constructor)]
    pub fn new(
        project: String,
        path: Option<String>,
        paths: Option<Vec<String>>,
        access_token: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(path, paths, None)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        Ok(Self {
            described: format!("firestore project {project}"),
            project,
            shape: Some(shape),
            access_token,
            timeout: seconds(timeout_ms),
        })
    }

    /// Reads the document. A network round trip, on a worker thread.
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> AsyncTask<Blocking> {
        let mut source = dynamic_config_firestore::Firestore::new(
            self.project.clone(),
            match self.shape.as_ref() {
                Some(Shape::Several(paths)) => {
                    dynamic_config_firestore::Keys::several(paths.clone())
                }
                Some(Shape::One(path)) => dynamic_config_firestore::Keys::one(path.clone()),
                _ => dynamic_config_firestore::Keys::one(String::new()),
            },
        );

        if let Some(token) = &self.access_token {
            source = source.with_auth(dynamic_config_firestore::Auth::access_token(token.clone()));
        }
        if let Some(timeout) = self.timeout {
            source = source.with_timeout(timeout);
        }

        AsyncTask::new(Blocking(Box::new(source)))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

// ── git ───────────────────────────────────────────────────────────────

/// A file in a git repository, at a branch, a tag or a commit.
#[napi]
pub struct Git {
    url: String,
    shape: Option<Shape>,
    reference: Option<(String, String)>,
    format: Option<Format>,
    token: Option<String>,
    timeout: Option<Duration>,
    described: String,
}

#[napi]
impl Git {
    /// `{ url, path | paths | prefix, branch? | tag? | commit?, format?, token?, timeoutMs? }`
    #[napi(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        url: String,
        path: Option<String>,
        paths: Option<Vec<String>>,
        prefix: Option<String>,
        branch: Option<String>,
        tag: Option<String>,
        commit: Option<String>,
        format: Option<String>,
        token: Option<String>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Self> {
        let shape = Shape::of(path, paths, prefix)
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;
        let format = match format {
            Some(name) => Some(
                format_of(&name).map_err(|error| napi::Error::from_reason(error.to_string()))?,
            ),
            None => None,
        };

        let reference = match (branch, tag, commit) {
            (Some(branch), None, None) => Some(("branch".to_owned(), branch)),
            (None, Some(tag), None) => Some(("tag".to_owned(), tag)),
            (None, None, Some(commit)) => Some(("commit".to_owned(), commit)),
            (None, None, None) => None,
            _ => {
                return Err(napi::Error::from_reason(
                    "a git source reads one `branch`, one `tag` or one \
                     `commit` — not two of them",
                ))
            }
        };

        Ok(Self {
            described: format!(
                "git {}",
                dynamic_config_store_core::redacted(
                    &url,
                    dynamic_config_store_core::LoneAuthority::Username,
                )
            ),
            url,
            shape: Some(shape),
            reference,
            format,
            token,
            timeout: seconds(timeout_ms),
        })
    }

    /// Fetches the repository shallowly and reads the file(s).
    #[napi(ts_return_type = "Promise<Outcome<Document>>")]
    pub fn fetch(&self) -> napi::Result<AsyncTask<Blocking>> {
        let mut builder = dynamic_config_git::GitSource::builder(self.url.clone()).path(match self
            .shape
            .as_ref()
        {
            Some(Shape::One(path)) => dynamic_config_git::Keys::one(path.clone()),
            Some(Shape::Several(paths)) => dynamic_config_git::Keys::several(paths.clone()),
            Some(Shape::Prefix(prefix)) => dynamic_config_git::Keys::prefix(prefix.clone()),
            None => dynamic_config_git::Keys::one(String::new()),
        });

        if let Some((kind, name)) = &self.reference {
            builder = match kind.as_str() {
                "branch" => builder.branch(name.clone()),
                "tag" => builder.tag(name.clone()),
                _ => builder.commit(name.clone()),
            };
        }
        if let Some(format) = self.format {
            builder = builder.format(format);
        }
        if let Some(token) = &self.token {
            builder = builder.credential(dynamic_config_git::Credential::token(token.clone()));
        }
        if let Some(timeout) = self.timeout {
            builder = builder.with_timeout(timeout);
        }

        let source = builder
            .build()
            .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        Ok(AsyncTask::new(Blocking(Box::new(source))))
    }

    /// How this store names itself in an error or a report.
    #[napi]
    pub fn describe(&self) -> String {
        self.described.clone()
    }
}

/// The package's own version.
#[napi(js_name = "packageVersion")]
pub fn package_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// The engine this package was built against.
#[napi(js_name = "engineVersion")]
pub fn engine_version() -> String {
    dynamic_config::VERSION.to_owned()
}
