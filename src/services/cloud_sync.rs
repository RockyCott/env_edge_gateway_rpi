use crate::config::Config;
use crate::database::Database;
use crate::models::{CloudHeader, CloudPayload, SensorMetric};
use chrono::Utc;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::sync::Arc;
use std::time::Duration;

/// Servicio de sincronización con el cloud principal via MQTT
/// Maneja el envío de datos procesados al servicio central
pub struct CloudSync {
    config: Arc<Config>,
    mqtt_client: Option<AsyncClient>,
}

impl CloudSync {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            mqtt_client: None,
        }
    }

    /// Inicializa la conexión MQTT con el cloud
    async fn init_mqtt_client(&mut self) -> anyhow::Result<AsyncClient> {
        let mut mqttoptions = MqttOptions::new(
            &self.config.cloud_mqtt_client_id,
            &self.config.cloud_mqtt_broker_host,
            self.config.cloud_mqtt_broker_port,
        );

        mqttoptions.set_keep_alive(Duration::from_secs(60));
        mqttoptions.set_max_packet_size(2 * 1024 * 1024, 2 * 1024 * 1024); // 2MB

        // Autenticación si está configurada
        if let (Some(username), Some(password)) = (
            &self.config.cloud_mqtt_username,
            &self.config.cloud_mqtt_password,
        ) {
            mqttoptions.set_credentials(username, password);
        }

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);

        tracing::info!(
            broker = %self.config.cloud_mqtt_broker_host,
            port = self.config.cloud_mqtt_broker_port,
            "Conectando a broker MQTT del cloud"
        );

        // Iniciar eventloop en background
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Error en MQTT eventloop del cloud: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        // Pequeña espera para asegurar conexión
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(client)
    }

    /// Sincroniza datos pendientes con el cloud via MQTT
    pub async fn sync_data(&mut self, db: Database) -> anyhow::Result<()> {
        tracing::info!("Iniciando sincronización con cloud via MQTT");

        // Obtener datos pendientes de sincronizar
        let pending_data = db
            .get_pending_sync(self.config.cloud_sync_batch_size as usize)
            .await?;

        if pending_data.is_empty() {
            tracing::debug!("No hay datos pendientes de sincronización");
            return Ok(());
        }

        // Asegurar cliente MQTT inicializado
        if self.mqtt_client.is_none() {
            self.mqtt_client = Some(self.init_mqtt_client().await?);
        }

        let client = self.mqtt_client.as_ref().unwrap();

        // Enviar cada dato procesado como mensaje individual
        let mut sent_count = 0;
        let mut failed_ids = Vec::new();

        for data in &pending_data {
            match self.send_to_cloud_mqtt(client, data).await {
                Ok(_) => {
                    sent_count += 1;
                }
                Err(e) => {
                    tracing::error!(
                        id = %data.id,
                        error = %e,
                        "Error enviando dato al cloud"
                    );
                    failed_ids.push(data.id);
                }
            }

            // Pequeño delay para no saturar el broker
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Marcar como sincronizados solo los que se enviaron exitosamente
        if sent_count > 0 {
            let successful_ids: Vec<_> = pending_data
                .iter()
                .filter(|d| !failed_ids.contains(&d.id))
                .map(|d| d.id)
                .collect();

            db.mark_as_synced(&successful_ids).await?;

            tracing::info!(
                sent = sent_count,
                failed = failed_ids.len(),
                "Sincronización completada via MQTT"
            );
        }

        if !failed_ids.is_empty() {
            anyhow::bail!("Falló el envío de {} mensajes", failed_ids.len());
        }

        Ok(())
    }

    /// Envía un dato procesado al cloud via MQTT
    async fn send_to_cloud_mqtt(
        &self,
        client: &AsyncClient,
        data: &crate::models::ProcessedSensorData,
    ) -> anyhow::Result<()> {
        // Construir header con UUID del usuario del gateway
        let cloud_header = CloudHeader {
            user_uuid: self.config.user_uuid.clone(),
            device_id: data.header.device_id.clone(),
            location: data.header.location.clone(),
            topic: data.header.topic.clone(),
            should_requeue: data.header.should_requeue,
            gateway_id: self.config.gateway_id.clone(),
        };

        // Construir métricas incluyendo las computadas si existen
        let mut all_metrics = data.metrics.clone();

        // Agregar métricas computadas como métricas adicionales
        if let Some(hi) = data.computed.heat_index {
            all_metrics.push(SensorMetric {
                measurement: "HeatIndex".to_string(),
                value: hi,
            });
        }

        if let Some(dp) = data.computed.dew_point {
            all_metrics.push(SensorMetric {
                measurement: "DewPoint".to_string(),
                value: dp,
            });
        }

        if let Some(cl) = data.computed.comfort_level {
            all_metrics.push(SensorMetric {
                measurement: "ComfortLevel".to_string(),
                value: cl,
            });
        }

        // Agregar quality score como métrica
        all_metrics.push(SensorMetric {
            measurement: "QualityScore".to_string(),
            value: data.quality.score as f32,
        });

        // Construir payload
        let payload = CloudPayload {
            header: cloud_header,
            metrics: all_metrics,
            sent_at: Utc::now(),
            quality: data.quality.clone(),
        };

        // Serializar a JSON
        let payload_json = serde_json::to_string(&payload)?;

        // Publicar en el topic del cloud
        client
            .publish(
                &self.config.cloud_mqtt_topic,
                QoS::AtLeastOnce,
                false,
                payload_json.as_bytes(),
            )
            .await?;

        tracing::debug!(
            device_id = %data.header.device_id,
            topic = %self.config.cloud_mqtt_topic,
            "Dato enviado al cloud via MQTT"
        );

        Ok(())
    }

    /// Tarea periódica de sincronización
    pub async fn start_sync_task(&mut self, db: Database) {
        let interval_secs = self.config.cloud_sync_interval_secs;
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

        tracing::info!(
            interval_secs = interval_secs,
            "Tarea de sincronización periódica iniciada (MQTT)"
        );

        loop {
            interval.tick().await;

            if let Err(e) = self.sync_data(db.clone()).await {
                tracing::error!("Error en sincronización periódica: {}", e);
            }
        }
    }

    /// Intenta resincronizar datos que fallaron previamente
    pub async fn retry_failed_syncs(&mut self, db: Database) -> anyhow::Result<()> {
        tracing::info!("Reintentando sincronizaciones fallidas");
        self.sync_data(db).await
    }
}
