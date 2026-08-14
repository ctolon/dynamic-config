//! The one place JavaScript is entered on a load, and the thread rule that
//! makes it safe.
//!
//! Node's rule is that only the loop thread may touch a JavaScript value.
//! The engine's rule is that validation happens *inside* the load, before
//! anything installs — that is what makes a rejected edit change nothing.
//! The two meet here: the load runs on a worker thread (libuv's pool, or
//! the file watcher's own), and when it reaches the validate hook it hands
//! the resolved document to the loop through a [`ThreadsafeFunction`] and
//! **blocks until the answer comes back**.
//!
//! Blocking a worker on the loop is safe in exactly one direction. It is
//! why every load in this binding is asynchronous: a synchronous `init()`
//! would be the loop thread waiting for itself, which is a deadlock at
//! startup — the worst place to put one. `initSync` is therefore not in
//! 0.0.1, and the note in the README says so rather than leaving somebody
//! to discover it.
//!
//! What crosses is a plain object each way. No JavaScript reference is
//! held by Rust past the call, which is what lets the validated document
//! live in a `serde_json::Value` and the watcher thread own nothing.

use std::sync::mpsc;
use std::time::Duration;

use dynamic_config::Error;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use serde_json::Value;

/// How long a validation may take before the load gives up on the loop.
///
/// Not a policy about slow validators — a schema check is microseconds —
/// but a guarantee that a watcher thread cannot be parked forever by a
/// loop that has gone away, which is what an exiting process looks like
/// from here.
const PATIENCE: Duration = Duration::from_secs(30);

/// A JavaScript function the engine may call from any thread.
///
/// Named because the parameter list is six long and appears three times:
/// the argument, the return, the raw argument, the error status, whether
/// the callee handles errors (it does not — this crate reads the `Result`)
/// and whether the reference is weak (it is, so a configuration does not
/// keep the process alive).
pub(crate) type Callable<T, R> = ThreadsafeFunction<T, R, T, napi::Status, false, true>;

/// The validator, as the engine's hook sees it.
pub(crate) struct Door {
    /// `None` for a configuration with no schema: the document *is* the
    /// value, and there is nothing to call.
    validate: Option<Callable<Value, Value>>,
}

impl Door {
    pub(crate) fn new(validate: Option<Callable<Value, Value>>) -> Self {
        Self { validate }
    }

    /// One resolved document, validated by whatever the caller declared.
    ///
    /// Answers the validated value — which is what installs — or the
    /// message the validator refused with. The message is the validator's
    /// own: a schema library says *why* far better than this binding
    /// could, and unlike Pydantic's report it is a string in every
    /// ecosystem Node has.
    pub(crate) fn validate(&self, tree: &Value) -> Result<Value, Error> {
        let Some(validate) = &self.validate else {
            return Ok(tree.clone());
        };

        let (answer, wait) = mpsc::channel();

        let status = validate.call_with_return_value(
            tree.clone(),
            ThreadsafeFunctionCallMode::Blocking,
            move |outcome, _env| {
                // The send can only fail if the load gave up waiting, which
                // `PATIENCE` below is the only way to do — and a validated
                // document nobody is waiting for is not an error, it is a
                // process shutting down.
                let _ = answer.send(match outcome {
                    Ok(value) => Ok(value),
                    Err(failure) => Err(failure.reason.clone()),
                });

                Ok(())
            },
        );

        if status != napi::Status::Ok {
            return Err(Error::invalid(format!(
                "the validator could not be reached ({status}); the event \
                 loop this configuration was created on has gone away"
            )));
        }

        match wait.recv_timeout(PATIENCE) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(message)) => Err(Error::invalid(message)),
            Err(_) => Err(Error::invalid(
                "the validator did not answer within 30 seconds; a \
                 validation is a function call, so this is an event loop \
                 that has stopped rather than a slow schema",
            )),
        }
    }
}
