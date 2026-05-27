use std::net::SocketAddr;

use ai_wallet::build_app;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ai_wallet=info,tower_http=info".into()),
        )
        .init();

    let app = build_app();
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(addr)
        .await
        .expect("bind ai-wallet listener");

    info!(
        "ai-wallet listening on http://{}",
        listener.local_addr().expect("local addr")
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve ai-wallet");
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
