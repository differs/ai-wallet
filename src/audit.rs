use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    error::AppError,
    model::{AuditEvent, AuditEventInput},
};

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn record(&self, event: AuditEventInput) -> Result<AuditEvent, AppError>;
    async fn list(&self, limit: usize) -> Result<Vec<AuditEvent>, AppError>;
    fn backend_name(&self) -> &'static str;
}

pub struct MemoryAuditRepository {
    events: Mutex<Vec<AuditEvent>>,
}

impl MemoryAuditRepository {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl AuditRepository for MemoryAuditRepository {
    async fn record(&self, event: AuditEventInput) -> Result<AuditEvent, AppError> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            request_id: event.request_id,
            tenant_id: event.tenant_id,
            wallet_id: event.wallet_id,
            actor: event.actor,
            action: event.action,
            status: event.status,
            metadata: event.metadata,
            created_at: Utc::now(),
        };
        self.events.lock().await.push(event.clone());
        Ok(event)
    }

    async fn list(&self, limit: usize) -> Result<Vec<AuditEvent>, AppError> {
        let events = self.events.lock().await;
        Ok(events.iter().rev().take(limit).cloned().collect())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}

pub struct PostgresAuditRepository {
    pool: PgPool,
}

impl PostgresAuditRepository {
    pub async fn connect(database_url: &str) -> Result<Self, AppError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| AppError::Internal(format!("failed to connect postgres: {e}")))?;

        sqlx::query(
            r#"
            create table if not exists audit_events (
                id uuid primary key,
                request_id uuid null,
                tenant_id text null,
                wallet_id text null,
                actor text null,
                action text not null,
                status text not null,
                metadata jsonb not null,
                created_at timestamptz not null
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to prepare audit schema: {e}")))?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl AuditRepository for PostgresAuditRepository {
    async fn record(&self, event: AuditEventInput) -> Result<AuditEvent, AppError> {
        let created = Utc::now();
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into audit_events
                (id, request_id, tenant_id, wallet_id, actor, action, status, metadata, created_at)
            values
                ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(event.request_id)
        .bind(&event.tenant_id)
        .bind(&event.wallet_id)
        .bind(&event.actor)
        .bind(&event.action)
        .bind(&event.status)
        .bind(&event.metadata)
        .bind(created)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to record audit event: {e}")))?;

        Ok(AuditEvent {
            id,
            request_id: event.request_id,
            tenant_id: event.tenant_id,
            wallet_id: event.wallet_id,
            actor: event.actor,
            action: event.action,
            status: event.status,
            metadata: event.metadata,
            created_at: created,
        })
    }

    async fn list(&self, limit: usize) -> Result<Vec<AuditEvent>, AppError> {
        let rows = sqlx::query(
            r#"
            select id, request_id, tenant_id, wallet_id, actor, action, status, metadata, created_at
            from audit_events
            order by created_at desc
            limit $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to list audit events: {e}")))?;

        rows.into_iter()
            .map(|row| {
                Ok(AuditEvent {
                    id: row.try_get("id")?,
                    request_id: row.try_get("request_id")?,
                    tenant_id: row.try_get("tenant_id")?,
                    wallet_id: row.try_get("wallet_id")?,
                    actor: row.try_get("actor")?,
                    action: row.try_get("action")?,
                    status: row.try_get("status")?,
                    metadata: row.try_get("metadata")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(|e| AppError::Internal(format!("failed to decode audit events: {e}")))
    }

    fn backend_name(&self) -> &'static str {
        "postgres"
    }
}

pub async fn build_audit_repository(
    database_url: Option<&str>,
) -> Result<Arc<dyn AuditRepository>, AppError> {
    match database_url {
        Some(url) => Ok(Arc::new(PostgresAuditRepository::connect(url).await?)),
        None => Ok(Arc::new(MemoryAuditRepository::new())),
    }
}
