use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::{
    config::Config, database::Database, models::SensorDataInput, services::cloud_sync::CloudSync,
    services::edge_processor::EdgeProcessor,
};

/// Handler MQTT para recibir datos de sensores ESP32
/// Los sensores publican en topics: sensors/{sensor_id}/data
pub struct MqttHandler {
    client: AsyncClient,
    config: Arc<Config>,
    db: Database,
    edge_processor: Arc<EdgeProcessor>,
    cloud_sync: Arc<CloudSync>,
}

impl MqttHandler {
    /// Crea una nueva instancia del handler MQTT
    pub async fn new(
        config: Arc<Config>,
        db: Database,
        edge_processor: Arc<EdgeProcessor>,
        cloud_sync: Arc<CloudSync>,
    ) -> anyhow::Result<Self> {
        // Configurar opciones MQTT
        let mut mqttoptions = MqttOptions::new(
            &config.mqtt_client_id,
            &config.mqtt_broker_host,
            config.mqtt_broker_port,
        );

        mqttoptions.set_keep_alive(Duration::from_secs(60));
        mqttoptions.set_max_packet_size(1024 * 1024, 1024 * 1024); // 1MB

        // Autenticación si está configurada
        if let (Some(username), Some(password)) = (&config.mqtt_username, &config.mqtt_password) {
            mqttoptions.set_credentials(username, password);
        }

        // Crear cliente async
        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);

        tracing::info!(
            broker = %config.mqtt_broker_host,
            port = config.mqtt_broker_port,
            "Conectando a broker MQTT"
        );

        // Suscribirse a topics
        // sensors/+/data - Datos individuales de cualquier sensor
        // sensors/+/batch - Batches de datos
        client.subscribe("sensors/+/data", QoS::AtLeastOnce).await?;
        client
            .subscribe("sensors/+/batch", QoS::AtLeastOnce)
            .await?;

        tracing::info!("Suscrito a topics: sensors/+/data, sensors/+/batch");

        Ok(Self {
            client,
            config,
            db,
            edge_processor,
            cloud_sync,
        })
    }

    /// Inicia el loop de procesamiento de mensajes MQTT
    pub async fn start(self) -> JoinHandle<()> {
        let (client, mut eventloop) = {
            let mut mqttoptions = MqttOptions::new(
                &self.config.mqtt_client_id,
                &self.config.mqtt_broker_host,
                self.config.mqtt_broker_port,
            );

            mqttoptions.set_keep_alive(Duration::from_secs(60));
            mqttoptions.set_max_packet_size(1024 * 1024, 1024 * 1024);

            if let (Some(username), Some(password)) =
                (&self.config.mqtt_username, &self.config.mqtt_password)
            {
                mqttoptions.set_credentials(username, password);
            }

            AsyncClient::new(mqttoptions, 100)
        };

        // Suscribirse
        let subscribe_client = client.clone();
        tokio::spawn(async move {
            if let Err(e) = subscribe_client
                .subscribe("sensors/+/data", QoS::AtLeastOnce)
                .await
            {
                tracing::error!("Error suscribiéndose a sensors/+/data: {}", e);
            }
            if let Err(e) = subscribe_client
                .subscribe("sensors/+/batch", QoS::AtLeastOnce)
                .await
            {
                tracing::error!("Error suscribiéndose a sensors/+/batch: {}", e);
            }
        });

        let db = self.db.clone();
        let edge_processor = self.edge_processor.clone();
        let cloud_sync = self.cloud_sync.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            tracing::info!("MQTT Handler iniciado, escuchando mensajes...");

            loop {
                match eventloop.poll().await {
                    Ok(notification) => {
                        if let Event::Incoming(Packet::Publish(publish)) = notification {
                            let topic = publish.topic.clone();
                            let payload = publish.payload.to_vec();

                            tracing::debug!(
                                topic = %topic,
                                payload_size = payload.len(),
                                "Mensaje MQTT recibido"
                            );

                            // Procesar mensaje
                            if let Err(e) = Self::process_message(
                                &topic,
                                &payload,
                                db.clone(),
                                edge_processor.clone(),
                                cloud_sync.clone(),
                                config.clone(),
                                client.clone(),
                            )
                            .await
                            {
                                tracing::error!(
                                    topic = %topic,
                                    error = %e,
                                    "Error procesando mensaje MQTT"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error en MQTT eventloop: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        })
    }

    /// Procesa un mensaje MQTT recibido
    async fn process_message(
        topic: &str,
        payload: &[u8],
        db: Database,
        edge_processor: Arc<EdgeProcessor>,
        cloud_sync: Arc<CloudSync>,
        config: Arc<Config>,
        client: AsyncClient,
    ) -> anyhow::Result<()> {
        // Parsear topic para obtener sensor_id y tipo
        let parts: Vec<&str> = topic.split('/').collect();

        if parts.len() < 3 {
            tracing::warn!("Topic inválido: {}", topic);
            return Ok(());
        }

        let sensor_id = parts[1];
        let message_type = parts[2]; // "data" o "batch"

        match message_type {
            "data" => {
                // Procesar dato individual
                Self::process_single_data(
                    sensor_id,
                    payload,
                    db.clone(),
                    edge_processor.clone(),
                    cloud_sync.clone(),
                    config.clone(),
                    client.clone(),
                )
                .await?;
            }
            "batch" => {
                // Procesar batch
                Self::process_batch_data(
                    sensor_id,
                    payload,
                    db.clone(),
                    edge_processor.clone(),
                    cloud_sync.clone(),
                    config.clone(),
                    client.clone(),
                )
                .await?;
            }
            _ => {
                tracing::warn!("Tipo de mensaje desconocido: {}", message_type);
            }
        }

        Ok(())
    }

    /// Procesa un dato individual
    async fn process_single_data(
        sensor_id: &str,
        payload: &[u8],
        db: Database,
        edge_processor: Arc<EdgeProcessor>,
        cloud_sync: Arc<CloudSync>,
        config: Arc<Config>,
        client: AsyncClient,
    ) -> anyhow::Result<()> {
        // Deserializar payload JSON
        let mut input: SensorDataInput = serde_json::from_slice(payload)?;

        // Asegurar que el sensor_id del payload coincida con el topic
        input.sensor_id = sensor_id.to_string();

        tracing::info!(
            sensor_id = %sensor_id,
            temperature = %input.temperature,
            humidity = %input.humidity,
            "Dato recibido vía MQTT"
        );

        // Procesar con edge computing
        let processed = edge_processor.process_reading(input).await;

        if processed.computed.is_anomaly {
            tracing::warn!(
                sensor_id = %sensor_id,
                "Anomalía detectada vía MQTT"
            );
        }

        // Almacenar en base de datos
        db.insert_reading(&processed).await?;

        // Publicar respuesta con métricas procesadas
        let response_topic = format!("sensors/{}/processed", sensor_id);
        let response_payload = serde_json::json!({
            "id": processed.id,
            "gateway_timestamp": processed.gateway_timestamp,
            "computed_metrics": {
                "heat_index": processed.computed.heat_index,
                "dew_point": processed.computed.dew_point,
                "comfort_level": processed.computed.comfort_level,
                "is_anomaly": processed.computed.is_anomaly,
            },
            "quality_score": processed.quality.score,
        });

        if let Ok(payload_str) = serde_json::to_string(&response_payload) {
            let _ = client
                .publish(
                    response_topic,
                    QoS::AtMostOnce,
                    false,
                    payload_str.as_bytes(),
                )
                .await;
        }

        // Verificar si es necesario sincronizar
        let pending_count = db.count_pending_sync().await?;
        if pending_count >= config.cloud_sync_batch_size as i64 {
            tracing::info!("Iniciando sincronización con cloud");
            let cloud_sync_clone = cloud_sync.clone();
            let db_clone = db.clone();
            tokio::spawn(async move {
                if let Err(e) = cloud_sync_clone.sync_data(db_clone).await {
                    tracing::error!("Error en sincronización: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Procesa un batch de datos
    async fn process_batch_data(
        sensor_id: &str,
        payload: &[u8],
        db: Database,
        edge_processor: Arc<EdgeProcessor>,
        cloud_sync: Arc<CloudSync>,
        config: Arc<Config>,
        client: AsyncClient,
    ) -> anyhow::Result<()> {
        // Deserializar batch
        #[derive(serde::Deserialize)]
        struct BatchPayload {
            readings: Vec<SensorDataInput>,
        }

        let mut batch: BatchPayload = serde_json::from_slice(payload)?;

        // Asegurar sensor_id en todas las lecturas
        for reading in &mut batch.readings {
            reading.sensor_id = sensor_id.to_string();
        }

        let batch_size = batch.readings.len();
        tracing::info!(
            sensor_id = %sensor_id,
            batch_size = batch_size,
            "Batch recibido vía MQTT"
        );

        // Procesar batch
        let processed_batch = edge_processor.process_batch(batch.readings).await;

        // Estadísticas
        let mut anomalies = 0;
        let mut total_quality: u32 = 0;

        for data in &processed_batch {
            if data.computed.is_anomaly {
                anomalies += 1;
            }
            total_quality += data.quality.score as u32;
        }

        let avg_quality = if batch_size > 0 {
            total_quality as f32 / batch_size as f32
        } else {
            0.0
        };

        // Almacenar batch
        db.insert_batch(&processed_batch).await?;

        tracing::info!(
            sensor_id = %sensor_id,
            processed = batch_size,
            anomalies = anomalies,
            avg_quality = %avg_quality,
            "Batch procesado vía MQTT"
        );

        // Publicar respuesta
        let response_topic = format!("sensors/{}/batch_processed", sensor_id);
        let response_payload = serde_json::json!({
            "status": "success",
            "processed_count": batch_size,
            "anomalies_detected": anomalies,
            "average_quality_score": avg_quality,
        });

        if let Ok(payload_str) = serde_json::to_string(&response_payload) {
            let _ = client
                .publish(
                    response_topic,
                    QoS::AtMostOnce,
                    false,
                    payload_str.as_bytes(),
                )
                .await;
        }

        // Verificar sincronización
        let pending_count = db.count_pending_sync().await?;
        if pending_count >= config.cloud_sync_batch_size as i64 {
            let cloud_sync_clone = cloud_sync.clone();
            let db_clone = db.clone();
            tokio::spawn(async move {
                let _ = cloud_sync_clone.sync_data(db_clone).await;
            });
        }

        Ok(())
    }
}
