use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: String,
    pub auth_mode: AuthMode,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub auth_max_skew_seconds: i64,
    pub database_url: Option<String>,
    pub signer_mode: SignerMode,
    pub dev_private_key: Option<String>,
    pub signer_url: Option<String>,
    pub signer_client_cert_path: Option<String>,
    pub signer_client_key_path: Option<String>,
    pub signer_ca_cert_path: Option<String>,
    pub signer_bind_addr: String,
    pub signer_tls_cert_path: Option<String>,
    pub signer_tls_key_path: Option<String>,
    pub signer_client_ca_cert_path: Option<String>,
    pub rpc_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Disabled,
    Hmac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignerMode {
    Mock,
    LocalDev,
    RemoteMtls,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let auth_mode = match env::var("AI_WALLET_AUTH_MODE")
            .unwrap_or_else(|_| "disabled".into())
            .to_lowercase()
            .as_str()
        {
            "hmac" => AuthMode::Hmac,
            _ => AuthMode::Disabled,
        };

        let signer_mode = match env::var("AI_WALLET_SIGNER_MODE")
            .unwrap_or_else(|_| "mock".into())
            .to_lowercase()
            .as_str()
        {
            "local" | "local-dev" | "local_dev" => SignerMode::LocalDev,
            "remote-mtls" | "remote_mtls" | "remote" => SignerMode::RemoteMtls,
            _ => SignerMode::Mock,
        };

        Self {
            bind_addr: env::var("AI_WALLET_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into()),
            auth_mode,
            api_key: env::var("AI_WALLET_API_KEY").ok(),
            api_secret: env::var("AI_WALLET_API_SECRET").ok(),
            auth_max_skew_seconds: env::var("AI_WALLET_AUTH_MAX_SKEW_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            database_url: env::var("DATABASE_URL").ok(),
            signer_mode,
            dev_private_key: env::var("AI_WALLET_DEV_PRIVATE_KEY").ok(),
            signer_url: env::var("AI_WALLET_SIGNER_URL").ok(),
            signer_client_cert_path: env::var("AI_WALLET_SIGNER_CLIENT_CERT_PATH").ok(),
            signer_client_key_path: env::var("AI_WALLET_SIGNER_CLIENT_KEY_PATH").ok(),
            signer_ca_cert_path: env::var("AI_WALLET_SIGNER_CA_CERT_PATH").ok(),
            signer_bind_addr: env::var("AI_WALLET_SIGNER_BIND")
                .unwrap_or_else(|_| "127.0.0.1:9443".into()),
            signer_tls_cert_path: env::var("AI_WALLET_SIGNER_TLS_CERT_PATH").ok(),
            signer_tls_key_path: env::var("AI_WALLET_SIGNER_TLS_KEY_PATH").ok(),
            signer_client_ca_cert_path: env::var("AI_WALLET_SIGNER_CLIENT_CA_CERT_PATH").ok(),
            rpc_url: env::var("AI_WALLET_RPC_URL").ok(),
        }
    }
}
