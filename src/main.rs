//! A minimal reproducer for an odd Firefox hang.
//!
//! 1. Run with `cargo run`
//! 2. Open Firefox
//! 3. Open the Network tab in the developer tools
//! 4. Disable the browser cache (radio toggle, second row, near the end)
//! 5. Navigate to https://127.0.0.1:3000
//! 6. Acknowledge that these certs are self-signed, and load the page anyway
//! 7. Possibly see some stalled requests

use axum::body::Body;
use axum::extract::Request;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_extra::response::Css;
use eyre;
use hyper::body::Incoming;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder;
use hyper_util::service::TowerToHyperService;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt;
use tracing;
use tracing_subscriber;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();


    let router = Router::new()
        .route("/", get(async || Html(INDEX_HTML)))
        .route("/one.css", get(async || {
            // NB. Removing these sleeps seems to prevent the issue from triggering
            std::thread::sleep(std::time::Duration::from_millis(75));
            Css(ONE_CSS)
        }))
        .route("/two.css", get(async || {
            // NB. Removing these sleeps seems to prevent the issue from triggering
            std::thread::sleep(std::time::Duration::from_millis(50));
            Css(TWO_CSS)
        }));


    let certs = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("self_signed_certs")
        .join("cert.pem");
    let certs = CertificateDer::pem_file_iter(&certs)?.collect::<Result<Vec<_>, _>>()?;
    let key = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("self_signed_certs")
        .join("key.pem");
    let key = PrivateKeyDer::from_pem_file(&key)?;

    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    server_config.max_early_data_size = 1024;

    let server_config = Arc::new(server_config);


    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    tracing::info!("spawning www server on {listener:?}");


    loop {
        let (conn, peer_sa) = listener.accept().await?;
        tracing::info!(?peer_sa, "incoming connection established");

        let tower_service = router
            .clone()
            .map_request(move |req: Request<Incoming>| req.map(Body::new));
        let hyper_service = TowerToHyperService::new(tower_service);

        let server_config = server_config.clone();

        tokio::spawn(async move {
            let tls_acceptor = TlsAcceptor::from(server_config);
            let tls_conn = match tls_acceptor.accept(conn).await {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::error!(?err, ?peer_sa, "tls accept error");
                    return;
                }
            };
            tracing::info!("tls connection accepted");

            let tokio_conn = TokioIo::new(tls_conn);

            Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(tokio_conn, hyper_service)
                .await
                .unwrap_or_else(|err| {
                    tracing::error!(?err, "failed to handle request");
                });

            tracing::warn!("serve future resolved");
        });
    }
}

#[rustfmt::skip]
const INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html>

<head>
  <title>Axum TLS Hang</title>
  <meta content="text/html;charset=utf-8" http-equiv="Content-Type" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <meta charset="UTF-8" />
  <link rel="stylesheet" href="one.css" type="text/css">
  <link rel="stylesheet" href="two.css" type="text/css">
</head>

<body>
  <p id="main">Please disable the local cache, and check the network tab for hanging requests.</p>
</body>

</html>
"#;

#[rustfmt::skip]
const ONE_CSS: &str = r#"
body {
  background-color: oklch(0.279 0.041 260.031);
}
"#;

#[rustfmt::skip]
const TWO_CSS: &str = r#"
body {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas,
               "Liberation Mono", "Courier New", monospace;
  color: oklch(0.869 0.022 252.894);
}
"#;
