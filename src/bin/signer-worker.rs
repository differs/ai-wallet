use ai_wallet::{
    config::AppConfig,
    signer_rpc::{load_rustls_config, signer_worker_router},
};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ai_wallet=info,axum_server=info".into()),
        )
        .init();

    let config = AppConfig::from_env();
    let tls = load_rustls_config(
        config
            .signer_tls_cert_path
            .as_deref()
            .expect("AI_WALLET_SIGNER_TLS_CERT_PATH is required"),
        config
            .signer_tls_key_path
            .as_deref()
            .expect("AI_WALLET_SIGNER_TLS_KEY_PATH is required"),
        config
            .signer_client_ca_cert_path
            .as_deref()
            .expect("AI_WALLET_SIGNER_CLIENT_CA_CERT_PATH is required"),
    )
    .expect("load signer worker tls config");

    let app = signer_worker_router(config.dev_private_key.clone());
    let bind_addr = config
        .signer_bind_addr
        .parse()
        .expect("valid signer bind addr");
    info!(
        "signer-worker listening on https://{}",
        config.signer_bind_addr
    );

    axum_server::bind_rustls(bind_addr, tls)
        .serve(app.into_make_service())
        .await
        .expect("serve signer worker");
}
