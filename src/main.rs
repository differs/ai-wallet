use ai_wallet::{build_app, build_state, config::AppConfig};
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

    let config = AppConfig::from_env();
    let bind_addr = config.bind_addr.clone();
    let state = build_state(config).await.expect("build app state");
    let app = build_app(state);
    let listener = TcpListener::bind(&bind_addr)
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
