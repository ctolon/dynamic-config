//! The binary: read a configuration file, start a server, serve until asked
//! to stop.
//!
//! Everything with behaviour is in the library, so the whole of this file is
//! argument handling, a refusal printed where an operator will see it, and a
//! shutdown that lets in-flight requests finish. Anything worth testing is
//! tested where it lives.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::sync::Arc;

use dynamic_config::Builder;
use dynamic_config_server::{router, Server, ServerConfig, VERSION};

/// The section this server's own configuration lives under.
const SECTION: &str = "server";

const USAGE: &str = "\
usage: dynamic-config-server <config-file>

  <config-file>   TOML, JSON or YAML, holding a [server] section. See the
                  book for the keys; every one of them is documented on
                  `ServerConfig`.

options:
  -h, --help      print this
  -V, --version   print the version
";

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);

    let Some(first) = arguments.next() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };

    match first.as_str() {
        "-h" | "--help" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        "-V" | "--version" => {
            println!("dynamic-config-server {VERSION}");
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    if arguments.next().is_some() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }

    match run(&first) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dynamic-config-server: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config: ServerConfig = Builder::new(SECTION).file(path).load()?;

    // Before the runtime, before the socket: a configuration that would
    // serve the wrong thing to the wrong caller must not get as far as
    // holding an open port.
    let server = Arc::new(Server::start(&config)?);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve(server))
}

async fn serve(server: Arc<Server>) -> Result<(), Box<dyn std::error::Error>> {
    let address = server.address();
    let listener = tokio::net::TcpListener::bind(address).await?;

    // The posture on the first line, because "is this port encrypted" is the
    // first thing anybody asks of a config server and the answer should not
    // require reading the file that started it. It is one of three fixed
    // strings: never a path, never a subject, never a key.
    eprintln!(
        "dynamic-config-server {VERSION} listening on {address} (protection: {})",
        server.posture()
    );

    serve_on(listener, server).await
}

/// A config server is restarted by a rollout, which is a SIGTERM. Dropping
/// the fetch a pod is doing at that moment would make a rollout look like a
/// configuration failure to whoever is starting up beside it.
async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Plain HTTP, or TLS if this build has it and this configuration asked for
/// it. The router is the same object either way.
#[cfg(feature = "tls")]
async fn serve_on(
    listener: tokio::net::TcpListener,
    server: Arc<Server>,
) -> Result<(), Box<dyn std::error::Error>> {
    if server.tls().is_some() {
        dynamic_config_server::serve_tls(
            listener,
            router(Arc::clone(&server)),
            &server,
            shutdown(),
        )
        .await?;

        return Ok(());
    }

    axum::serve(listener, router(server))
        .with_graceful_shutdown(shutdown())
        .await?;

    Ok(())
}

/// A build without the `tls` feature has one serving path. A configuration
/// that asked for TLS never reaches here: `ServerConfig::validate` refused
/// it, naming the feature.
#[cfg(not(feature = "tls"))]
async fn serve_on(
    listener: tokio::net::TcpListener,
    server: Arc<Server>,
) -> Result<(), Box<dyn std::error::Error>> {
    axum::serve(listener, router(server))
        .with_graceful_shutdown(shutdown())
        .await?;

    Ok(())
}
