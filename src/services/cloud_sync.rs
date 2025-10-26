use crate::config::Config;
use crate::database::Database;
use crate::models::{BatchStatistics, CloudPayload};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

/// Servicio de sincronización con el cloud principal
/// Maneja el envío de datos procesados al servicio central
pub struct CloudSync {
    config: Arc<Config>,
    client: reqwest::Client,
}

impl CloudSync {
    pub fn new(config: Arc<Config>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Sincroniza datos pendientes con el cloud
    pub async fn sync_data(&self, db: Database) -> anyhow::Result<()> {
        tracing::info!("Iniciando sincronización con cloud");

        // Obtener datos pendientes de sincronizar
        let pending_data = db
            .get_pending_sync(self.config.cloud_sync_batch_size as usize)
            .await?;

        if pending_data.is_empty() {
            tracing::debug!("✓ No hay datos pendientes de sincronización");
            return Ok(());
        }

        // Calcular estadísticas del batch
        let stats = self.calculate_batch_stats(&pending_data);

        // Construir payload para el cloud
        let payload = CloudPayload {
            gateway_id: self.config.gateway_id.clone(),
            gateway_version: env!("CARGO_PKG_VERSION").to_string(),
            data: pending_data.clone(),
            sent_at: Utc::now(),
            batch_stats: stats,
        };

        // Enviar al servicio cloud
        match self.send_to_cloud(&payload).await {
            Ok(_) => {
                // Marcar como sincronizados
                let ids: Vec<_> = pending_data.iter().map(|d| d.id).collect();
                db.mark_as_synced(&ids).await?;

                tracing::info!(
                    count = pending_data.len(),
                    "Datos sincronizados exitosamente"
                );
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "Error al sincronizar con cloud"
                );

                // Los datos quedan pendientes para reintentar después
                return Err(e);
            }
        }

        Ok(())
    }

    /// Envía datos al servicio cloud principal
    async fn send_to_cloud(&self, payload: &CloudPayload) -> anyhow::Result<()> {
        let response = self
            .client
            .post(&self.config.cloud_service_url)
            .header("Content-Type", "application/json")
            .header("X-Gateway-ID", &self.config.gateway_id)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.cloud_api_key),
            )
            .json(payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Cloud service respondió con error {}: {}", status, body);
        }

        Ok(())
    }

    /// Calcula estadísticas del batch para enviar al cloud
    fn calculate_batch_stats(
        &self,
        data: &[crate::models::ProcessedSensorData],
    ) -> BatchStatistics {
        let total_readings = data.len() as u32;
        let anomalies_detected = data.iter().filter(|d| d.computed.is_anomaly).count() as u32;

        let unique_sensors: std::collections::HashSet<_> =
            data.iter().map(|d| &d.sensor_id).collect();
        let sensors_count = unique_sensors.len() as u32;

        let total_quality: u32 = data.iter().map(|d| d.quality.score as u32).sum();
        let avg_quality_score = if total_readings > 0 {
            total_quality as f32 / total_readings as f32
        } else {
            0.0
        };

        BatchStatistics {
            total_readings,
            anomalies_detected,
            sensors_count,
            avg_quality_score,
        }
    }

    /// Tarea periódica de sincronización
    /// Sincroniza cada X segundos automáticamente
    pub async fn start_sync_task(&self, db: Database) {
        let interval_secs = self.config.cloud_sync_interval_secs;
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

        tracing::info!(
            interval_secs = interval_secs,
            "Tarea de sincronización periódica iniciada"
        );

        loop {
            interval.tick().await;

            if let Err(e) = self.sync_data(db.clone()).await {
                tracing::error!("Error en sincronización periódica: {}", e);
            }
        }
    }

    /// Intenta resincronizar datos que fallaron previamente
    pub async fn retry_failed_syncs(&self, db: Database) -> anyhow::Result<()> {
        // Implementar lógica de retry con exponential backoff
        tracing::info!("Reintentando sincronizaciones fallidas");
        self.sync_data(db).await
    }
}
