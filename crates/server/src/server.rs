//! The HTTP surface. `router()` is pure, so unit tests drive it with `oneshot` and no socket;
//! `serve(listener, ..)` runs it on an already-bound listener, so tests bind `127.0.0.1:0`
//! in-process.

use std::{
    net::{IpAddr, SocketAddr},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context as _;
use axum::{
    Json, Router,
    extract::{
        ConnectInfo, Path as RoutePath, State,
        ws::{WebSocketUpgrade, rejection::WebSocketUpgradeRejection},
    },
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header::CACHE_CONTROL},
    response::{IntoResponse as _, Response},
    routing::{get, get_service, post},
};
use tokio::net::TcpListener;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use crate::{
    config::Config,
    connection::{self, MAX_FRAME},
    rate::{Limiter, Rate, TOO_FAST},
    room::Timing,
    rooms::Rooms,
};
use lobshot_assets::maps::Maps;
use lobshot_protocol::RoomCode;
use lobshot_sim::base::Void;

/// What the API handlers share.
#[derive(Debug, Clone)]
struct App {
    rooms: Rooms,
    /// Room creation, per client IP.
    creations: Arc<Limiter<IpAddr>>,
    client_ip_header: Option<HeaderName>,
}

/// Builds the routes: `/healthz`; `POST /api/rooms`, which opens a room and answers its code;
/// `/ws/{code}`, the room's WebSocket; the client bundle from `config.dist`; and `/r/{code}` room
/// links, which load the same page as `/` (the client reads the code from the URL). Each router
/// owns its own rooms.
///
/// Paths outside the bundle are a 404 rather than the page, so a mistyped asset fails loudly
/// instead of handing the browser HTML.
pub fn router(config: &Config) -> Router {
    let dist = config.dist.as_path();
    let index = ServeFile::new(dist.join("index.html")).precompressed_br().precompressed_gzip();
    let bundle = ServeDir::new(dist).precompressed_br().precompressed_gzip();

    // Every response revalidates. trunk hashes the wasm and JS names, but other files (maps, later)
    // keep theirs, and a heuristic cache could pair a new page with a stale map. A revalidation is
    // a cheap 304.
    let revalidate = SetResponseHeaderLayer::overriding(CACHE_CONTROL, HeaderValue::from_static("no-cache"));

    let pages = Router::new().route("/r/{code}", get_service(index)).fallback_service(bundle).layer(revalidate);

    let app = App {
        rooms: Rooms::new(
            Arc::new(Maps::new(&dist.join("assets"))),
            Timing {
                playback_wait: !config.no_playback_wait,
                ..Timing::new(Duration::from_millis(config.bot_delay_ms))
            },
        ),
        creations: Arc::new(Limiter::new(Rate::ROOMS)),
        client_ip_header: config.client_ip_header.clone(),
    };
    let api = Router::new().route("/api/rooms", post(create_room)).route("/ws/{code}", get(socket)).with_state(app);

    Router::new().route("/healthz", get(|| async { "ok" })).merge(api).merge(pages)
}

/// Opens a room: `201` with `{"code": "<code>"}`, or `429` past the per-IP limit.
async fn create_room(State(app): State<App>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> Response {
    let ip = client_ip(&headers, app.client_ip_header.as_ref(), peer);
    if !app.creations.take(ip, Instant::now()) {
        return (StatusCode::TOO_MANY_REQUESTS, TOO_FAST).into_response();
    }
    let code = app.rooms.create();
    (StatusCode::CREATED, Json(serde_json::json!({ "code": code }))).into_response()
}

/// Upgrades to a room's WebSocket. An unknown room is a 404 whether or not the request asked to
/// upgrade.
async fn socket(State(app): State<App>, RoutePath(code): RoutePath<String>, upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>) -> Response {
    let Some(room) = RoomCode::parse(&code).ok().and_then(|code| app.rooms.get(&code)) else {
        return (StatusCode::NOT_FOUND, "no such room").into_response();
    };
    match upgrade {
        Ok(upgrade) => {
            let conn = app.rooms.next_conn();
            upgrade
                .max_message_size(MAX_FRAME)
                .max_frame_size(MAX_FRAME)
                .on_upgrade(move |socket| connection::serve(socket, room, conn))
        }
        Err(rejection) => rejection.into_response(),
    }
}

/// The client's IP: from the configured proxy header when it holds one, else the peer address.
fn client_ip(headers: &HeaderMap, header: Option<&HeaderName>, peer: SocketAddr) -> IpAddr {
    header
        .and_then(|name| headers.get(name))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or_else(|| peer.ip())
}

/// Binds `config.bind` and serves until ctrl-c or SIGTERM.
///
/// # Errors
///
/// Fails when the address cannot be bound, or the server stops on an I/O error.
pub async fn run(config: &Config) -> Void {
    let listener = TcpListener::bind(config.bind).await.with_context(|| format!("binding {}", config.bind))?;
    serve(listener, config).await
}

/// Serves on an already-bound listener until ctrl-c or SIGTERM. `config.bind` is ignored.
///
/// # Errors
///
/// Fails when the server stops on an I/O error.
pub async fn serve(listener: TcpListener, config: &Config) -> Void {
    let addr = listener.local_addr().context("reading the bound address")?;
    let dist: &Path = &config.dist;
    // A missing bundle is not fatal: the API still works, and tests run without a client build.
    if !dist.join("index.html").is_file() {
        tracing::warn!(dist = %dist.display(), "no index.html in the client bundle; run `cargo make dist`");
    }
    tracing::info!(%addr, dist = %dist.display(), "listening");
    // axum's accept loop logs and continues on a per-connection error; only the listener itself
    // failing ends `serve`, so one bad connection never takes the service down. Upgraded sockets
    // leave the HTTP connection, so they don't hold up a graceful shutdown.
    axum::serve(listener, router(config).into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving")
}

/// Resolves on ctrl-c, or on SIGTERM (what `docker stop` and Fly send) on unix. If a handler
/// cannot be installed, that branch waits forever instead of shutting the server down at once.
async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use axum::extract::connect_info::MockConnectInfo;
    use axum::{
        body::Body,
        http::{
            Request, StatusCode,
            header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE},
            response::Parts,
        },
    };
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use tower::ServiceExt as _;

    use super::*;
    use crate::tests::{test_config, write_bundle};

    async fn request(dist: &Path, path: &str, accept_encoding: Option<&str>) -> (Parts, Vec<u8>) {
        let mut request = Request::get(path);
        if let Some(encoding) = accept_encoding {
            request = request.header(ACCEPT_ENCODING, encoding);
        }
        let response = router(&test_config(dist)).oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
        let (parts, body) = response.into_parts();
        (parts, axum::body::to_bytes(body, usize::MAX).await.unwrap().to_vec())
    }

    async fn get(dist: &Path, path: &str) -> (StatusCode, String) {
        let (parts, body) = request(dist, path, None).await;
        (parts.status, String::from_utf8(body).unwrap())
    }

    fn bundle() -> TempDir {
        let dist = TempDir::new().unwrap();
        write_bundle(dist.path()).unwrap();
        dist
    }

    #[tokio::test]
    async fn health_is_ok() {
        let dist = bundle();
        assert_eq!(get(dist.path(), "/healthz").await, (StatusCode::OK, "ok".to_owned()));
    }

    #[tokio::test]
    async fn serves_the_page_at_the_root() {
        let dist = bundle();
        let (status, body) = get(dist.path(), "/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("<canvas id=\"lobshot\">"), "{body}");
    }

    #[tokio::test]
    async fn room_links_load_the_same_page() {
        let dist = bundle();
        assert_eq!(get(dist.path(), "/r/k3xq9z").await, get(dist.path(), "/").await);
    }

    #[tokio::test]
    async fn paths_outside_the_bundle_are_not_found() {
        let dist = bundle();
        assert_eq!(get(dist.path(), "/missing.js").await.0, StatusCode::NOT_FOUND);
        assert_eq!(get(dist.path(), "/r/").await.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wasm_has_the_mime_type_streaming_compilation_needs() {
        let dist = bundle();
        let (parts, _) = request(dist.path(), "/lobshot-client_bg.wasm", None).await;
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.headers[CONTENT_TYPE], "application/wasm");
    }

    #[tokio::test]
    async fn every_bundle_response_revalidates() {
        let dist = bundle();
        for path in ["/", "/r/k3xq9z", "/lobshot-client_bg.wasm"] {
            let (parts, _) = request(dist.path(), path, None).await;
            assert_eq!(parts.headers[CACHE_CONTROL], "no-cache", "{path}");
        }
    }

    #[tokio::test]
    async fn serves_the_brotli_sibling_when_the_browser_accepts_it() {
        let dist = bundle();
        std::fs::write(dist.path().join("lobshot-client_bg.wasm.br"), b"pretend brotli").unwrap();

        let (parts, body) = request(dist.path(), "/lobshot-client_bg.wasm", Some("gzip, br")).await;
        assert_eq!(parts.headers[CONTENT_ENCODING], "br");
        assert_eq!(parts.headers[CONTENT_TYPE], "application/wasm");
        assert_eq!(body, b"pretend brotli");

        // Plain-HTTP self-hosts: browsers only offer br over HTTPS, so identity must still work.
        let (parts, _) = request(dist.path(), "/lobshot-client_bg.wasm", Some("gzip")).await;
        assert!(!parts.headers.contains_key(CONTENT_ENCODING));
    }

    /// POSTs to `/api/rooms` as `peer`, with extra headers; answers the status and body.
    async fn create(app: &Router, peer: &str, headers: &[(&str, &str)]) -> (StatusCode, String) {
        let mut request = Request::post("/api/rooms");
        for &(name, value) in headers {
            request = request.header(name, value);
        }
        let app = app.clone().layer(MockConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let response = app.oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn creating_a_room_answers_201_and_its_code() {
        let dist = bundle();
        let app = router(&test_config(dist.path()));
        let (status, body) = create(&app, "10.0.0.1:5000", &[]).await;
        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(RoomCode::parse(json["code"].as_str().unwrap()).is_ok(), "{body}");
    }

    #[tokio::test]
    async fn room_creation_is_limited_per_client_ip() {
        let dist = bundle();
        let app = router(&test_config(dist.path()));
        for _ in 0..10 {
            assert_eq!(create(&app, "10.0.0.1:5000", &[]).await.0, StatusCode::CREATED);
        }
        assert_eq!(
            create(&app, "10.0.0.1:5001", &[]).await,
            (StatusCode::TOO_MANY_REQUESTS, TOO_FAST.to_owned()),
            "the port doesn't matter"
        );
        assert_eq!(create(&app, "10.0.0.2:5000", &[]).await.0, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn behind_a_proxy_the_configured_header_names_the_client() {
        let dist = bundle();
        let mut config = test_config(dist.path());
        config.client_ip_header = Some(HeaderName::from_static("fly-client-ip"));
        let app = router(&config);
        // Every request comes from the proxy's address; the header tells clients apart.
        for _ in 0..10 {
            assert_eq!(create(&app, "10.0.0.1:5000", &[("Fly-Client-IP", "203.0.113.7")]).await.0, StatusCode::CREATED);
        }
        assert_eq!(create(&app, "10.0.0.1:5000", &[("Fly-Client-IP", "203.0.113.7")]).await.0, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(create(&app, "10.0.0.1:5000", &[("Fly-Client-IP", "203.0.113.8")]).await.0, StatusCode::CREATED);
        // Without a usable header, the peer address counts.
        assert_eq!(create(&app, "10.0.0.1:5000", &[("Fly-Client-IP", "garbage")]).await.0, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn sockets_for_unknown_rooms_are_not_found() {
        let dist = bundle();
        for path in ["/ws/aaaaaaaa", "/ws/not-a-code", "/ws/"] {
            assert_eq!(get(dist.path(), path).await.0, StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[tokio::test]
    async fn serves_the_api_without_a_bundle() {
        let dist = TempDir::new().unwrap();
        assert_eq!(get(dist.path(), "/healthz").await.0, StatusCode::OK);
        assert_eq!(get(dist.path(), "/").await.0, StatusCode::NOT_FOUND);
    }
}
