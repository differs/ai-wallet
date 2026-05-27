use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{config::AuthMode, error::AppError, service::AppState};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub enum ApiAuth {
    Disabled,
    Hmac(HmacAuthVerifier),
}

impl ApiAuth {
    pub fn mode_name(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Hmac(_) => "hmac",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HmacAuthVerifier {
    pub api_key: String,
    pub api_secret: String,
    pub max_skew_seconds: i64,
}

impl HmacAuthVerifier {
    pub fn from_config(
        mode: AuthMode,
        api_key: Option<String>,
        api_secret: Option<String>,
        max_skew_seconds: i64,
    ) -> Result<ApiAuth, AppError> {
        match mode {
            AuthMode::Disabled => Ok(ApiAuth::Disabled),
            AuthMode::Hmac => Ok(ApiAuth::Hmac(Self {
                api_key: api_key.ok_or_else(|| {
                    AppError::Internal("AI_WALLET_API_KEY is required for hmac auth".into())
                })?,
                api_secret: api_secret.ok_or_else(|| {
                    AppError::Internal("AI_WALLET_API_SECRET is required for hmac auth".into())
                })?,
                max_skew_seconds,
            })),
        }
    }

    fn verify(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Result<(), AppError> {
        let api_key = get_header(headers, "x-ai-wallet-key")?;
        if api_key != self.api_key {
            return Err(AppError::Unauthorized("invalid api key".into()));
        }

        let timestamp = get_header(headers, "x-ai-wallet-timestamp")?;
        let timestamp = timestamp
            .parse::<i64>()
            .map_err(|_| AppError::Unauthorized("invalid auth timestamp".into()))?;
        let signature = get_header(headers, "x-ai-wallet-signature")?;

        let now = Utc::now().timestamp();
        if (now - timestamp).abs() > self.max_skew_seconds {
            return Err(AppError::Unauthorized("stale request timestamp".into()));
        }

        let canonical = format!(
            "{}\n{}\n{}\n{}",
            timestamp,
            method,
            path,
            STANDARD.encode(body),
        );
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .map_err(|e| AppError::Internal(format!("failed to build hmac verifier: {e}")))?;
        mac.update(canonical.as_bytes());
        let expected = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        if signature != expected {
            return Err(AppError::Unauthorized("invalid request signature".into()));
        }

        Ok(())
    }
}

fn get_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, AppError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized(format!("missing header {name}")))
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    match &state.api_auth {
        ApiAuth::Disabled => Ok(next.run(request).await),
        ApiAuth::Hmac(verifier) => {
            let (parts, body) = request.into_parts();
            let bytes = to_bytes(body, 1024 * 1024)
                .await
                .map_err(|e| AppError::BadRequest(format!("failed to read request body: {e}")))?;
            verifier.verify(
                &parts.headers,
                parts.method.as_str(),
                parts.uri.path(),
                &bytes,
            )?;
            request = Request::from_parts(parts, Body::from(bytes));
            Ok(next.run(request).await)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_verifier_accepts_valid_signature() {
        let verifier = HmacAuthVerifier {
            api_key: "test-key".into(),
            api_secret: "test-secret".into(),
            max_skew_seconds: 300,
        };
        let timestamp = Utc::now().timestamp();
        let body = br#"{"a":1}"#;
        let canonical = format!(
            "{}\nPOST\n/v1/evm/sign-intent\n{}",
            timestamp,
            STANDARD.encode(body)
        );
        let mut mac = HmacSha256::new_from_slice(verifier.api_secret.as_bytes()).expect("hmac");
        mac.update(canonical.as_bytes());
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let mut headers = HeaderMap::new();
        headers.insert("x-ai-wallet-key", verifier.api_key.parse().expect("header"));
        headers.insert(
            "x-ai-wallet-timestamp",
            timestamp.to_string().parse().expect("header"),
        );
        headers.insert("x-ai-wallet-signature", signature.parse().expect("header"));

        verifier
            .verify(&headers, "POST", "/v1/evm/sign-intent", body)
            .expect("valid signature");
    }
}
