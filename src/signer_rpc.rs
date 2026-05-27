use std::{fs, io::BufReader, sync::Arc};

use crate::{
    error::AppError,
    model::{ExecutionMode, SignedArtifact, SignerRpcSignRequest, SignerRpcSignResponse},
    service::{LocalDevSigner, SignerEnvelope, SignerGateway},
};
use async_trait::async_trait;
use axum::{Json, Router, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use reqwest::{Certificate, Client, Identity};
use rustls::{
    RootCertStore, ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
};

#[derive(Clone)]
pub struct RemoteSignerClient {
    base_url: String,
    client: Client,
}

impl RemoteSignerClient {
    pub fn new(
        base_url: String,
        client_cert_path: &str,
        client_key_path: &str,
        ca_cert_path: &str,
    ) -> Result<Self, AppError> {
        let identity = Identity::from_pem(&combined_pem(client_cert_path, client_key_path)?)
            .map_err(|e| AppError::Internal(format!("failed to load client identity: {e}")))?;
        let ca = Certificate::from_pem(
            &fs::read(ca_cert_path)
                .map_err(|e| AppError::Internal(format!("failed to read signer CA cert: {e}")))?,
        )
        .map_err(|e| AppError::Internal(format!("failed to parse signer CA cert: {e}")))?;
        let client = Client::builder()
            .use_rustls_tls()
            .identity(identity)
            .add_root_certificate(ca)
            .https_only(true)
            .build()
            .map_err(|e| AppError::Internal(format!("failed to build signer client: {e}")))?;

        Ok(Self { base_url, client })
    }

    pub async fn request_signature(
        &self,
        request: SignerRpcSignRequest,
    ) -> Result<SignedArtifact, AppError> {
        let response = self
            .client
            .post(format!(
                "{}/v1/signer/sign",
                self.base_url.trim_end_matches('/')
            ))
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::SignerUnavailable(format!("signer RPC request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AppError::SignerUnavailable(format!(
                "signer RPC returned HTTP {}",
                response.status()
            )));
        }

        response
            .json::<SignerRpcSignResponse>()
            .await
            .map(|payload| payload.signed_artifact)
            .map_err(|e| AppError::SignerUnavailable(format!("invalid signer RPC response: {e}")))
    }
}

#[async_trait]
impl SignerGateway for RemoteSignerClient {
    async fn sign(&self, envelope: SignerEnvelope) -> Result<SignedArtifact, AppError> {
        self.request_signature(SignerRpcSignRequest {
            request_id: envelope.request_id,
            tenant_id: envelope.tenant_id,
            wallet_id: envelope.wallet_id,
            chain_id: envelope.chain_id,
            from_address: envelope.from_address,
            payload: envelope.payload,
        })
        .await
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::IsolatedSigner
    }
}

pub fn signer_worker_router(private_key: Option<String>) -> Router {
    let signer = Arc::new(LocalDevSigner::new(private_key));
    Router::new().route(
        "/v1/signer/sign",
        post(move |Json(request): Json<SignerRpcSignRequest>| {
            let signer = Arc::clone(&signer);
            async move {
                let signed_artifact = signer
                    .sign(SignerEnvelope {
                        request_id: request.request_id,
                        tenant_id: request.tenant_id,
                        wallet_id: request.wallet_id,
                        chain_id: request.chain_id,
                        from_address: request.from_address,
                        payload: request.payload,
                    })
                    .await?;
                Ok::<_, AppError>(Json(SignerRpcSignResponse { signed_artifact }))
            }
        }),
    )
}

pub fn load_rustls_config(
    cert_path: &str,
    key_path: &str,
    client_ca_cert_path: &str,
) -> Result<RustlsConfig, AppError> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;
    let roots = load_root_store(client_ca_cert_path)?;
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| AppError::Internal(format!("failed to build client cert verifier: {e}")))?;
    let server_config = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(certs, key)
        .map_err(|e| AppError::Internal(format!("failed to build TLS server config: {e}")))?;

    Ok(RustlsConfig::from_config(Arc::new(server_config)))
}

fn combined_pem(cert_path: &str, key_path: &str) -> Result<Vec<u8>, AppError> {
    let mut pem = fs::read(cert_path)
        .map_err(|e| AppError::Internal(format!("failed to read client cert PEM: {e}")))?;
    pem.extend_from_slice(
        &fs::read(key_path)
            .map_err(|e| AppError::Internal(format!("failed to read client key PEM: {e}")))?,
    );
    Ok(pem)
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, AppError> {
    let file = fs::File::open(path)
        .map_err(|e| AppError::Internal(format!("failed to open cert file `{path}`: {e}")))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Internal(format!("failed to parse cert file `{path}`: {e}")))
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, AppError> {
    let file = fs::File::open(path)
        .map_err(|e| AppError::Internal(format!("failed to open key file `{path}`: {e}")))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|e| AppError::Internal(format!("failed to parse key file `{path}`: {e}")))?
        .ok_or_else(|| AppError::Internal(format!("no private key found in `{path}`")))
}

fn load_root_store(path: &str) -> Result<RootCertStore, AppError> {
    let mut store = RootCertStore::empty();
    for cert in load_certs(path)? {
        store
            .add(cert)
            .map_err(|e| AppError::Internal(format!("failed to add CA cert from `{path}`: {e}")))?;
    }
    Ok(store)
}
