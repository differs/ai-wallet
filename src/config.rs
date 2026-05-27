use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: String,
    pub signer_mode: SignerMode,
    pub dev_private_key: Option<String>,
    pub rpc_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignerMode {
    Mock,
    LocalDev,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let signer_mode = match env::var("AI_WALLET_SIGNER_MODE")
            .unwrap_or_else(|_| "mock".into())
            .to_lowercase()
            .as_str()
        {
            "local" | "local-dev" | "local_dev" => SignerMode::LocalDev,
            _ => SignerMode::Mock,
        };

        Self {
            bind_addr: env::var("AI_WALLET_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into()),
            signer_mode,
            dev_private_key: env::var("AI_WALLET_DEV_PRIVATE_KEY").ok(),
            rpc_url: env::var("AI_WALLET_RPC_URL").ok(),
        }
    }
}
