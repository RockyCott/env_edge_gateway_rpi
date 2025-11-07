use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::{
    config::Config, database::Database, models::SensorDataInput, services::cloud_sync::CloudSync,
    services::edge_processor::EdgeProcessor,
};
use tokio::sync::Mutex;

/// Handler MQTT para recibir datos de sensores ESP32
/// Los sensores publican en topics: sensors/{device_id}/data
pub struct MqttHandler {
    client: AsyncClient,
    config: Arc<Config>,
    db: Database,
    edge_processor: Arc<EdgeProcessor>,
    cloud_sync: Arc<Mutex<CloudSync>>,
}

impl MqttHandler {
    /// Crea una nueva instancia del handler MQTT
    pub async fn new(
        config: Arc<Config>,
        db: Database,
        edge_processor: Arc<EdgeProcessor>,
        cloud_sync: Arc<Mutex<CloudSync>>,
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
        let (client, mut _eventloop) = AsyncClient::new(mqttoptions, 100);

        tracing::info!(
            broker = %config.mqtt_broker_host,
            port = config.mqtt_broker_port,
            "Conectando a broker MQTT local"
        );

        // Suscribirse a topics
        // sensors/+/data - Datos de cualquier sensor
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
        cloud_sync: Arc<Mutex<CloudSync>>,
        config: Arc<Config>,
        client: AsyncClient,
    ) -> anyhow::Result<()> {
        // Parsear topic para obtener device_id y tipo
        let parts: Vec<&str> = topic.split('/').collect();

        if parts.len() < 3 {
            tracing::warn!("Topic inválido: {}", topic);
            return Ok(());
        }

        let device_id = parts[1];
        let message_type = parts[2]; // "data" o "batch"

        match message_type {
            "data" => {
                Self::process_single_data(
                    device_id,
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
                Self::process_batch_data(
                    device_id,
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
        device_id: &str,
        payload: &[u8],
        db: Database,
        edge_processor: Arc<EdgeProcessor>,
        cloud_sync: Arc<Mutex<CloudSync>>,
        config: Arc<Config>,
        client: AsyncClient,
    ) -> anyhow::Result<()> {
        // Deserializar payload JSON con el nuevo formato
        let mut input: SensorDataInput = serde_json::from_slice(payload)?;

        // Asegurar que el device_id del header coincida con el topic
        input.header.device_id = device_id.to_string();

        tracing::info!(
            device_id = %device_id,
            location = %input.header.location,
            metrics_count = input.metrics.len(),
            "Dato recibido vía MQTT"
        );

        // Procesar con edge computing
        let processed = edge_processor.process_reading(input).await;

        if processed.computed.is_anomaly {
            tracing::warn!(
                device_id = %device_id,
                "Anomalía detectada vía MQTT"
            );
        }

        // Almacenar en base de datos
        db.insert_reading(&processed).await?;

        // Publicar respuesta con métricas procesadas
        let response_topic = format!("sensors/{}/processed", device_id);
        let response_payload = serde_json::json!({
            "id": processed.id,
            "gateway_timestamp": processed.gateway_timestamp,
            "computed_metrics": processed.computed,
            "quality_score": processed.quality.score,
            "quality_issues": processed.quality.issues,
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
            tokio::spawn(async move {
                let mut cs = cloud_sync.lock().await;
                if let Err(e) = cs.sync_data(db).await {
                    tracing::error!("Error en sincronización: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Procesa un batch de datos
    async fn process_batch_data(
        device_id: &str,
        payload: &[u8],
        db: Database,
        edge_processor: Arc<EdgeProcessor>,
        cloud_sync: Arc<Mutex<CloudSync>>,
        config: Arc<Config>,
        client: AsyncClient,
    ) -> anyhow::Result<()> {
        // Deserializar batch
        #[derive(serde::Deserialize)]
        struct BatchPayload {
            readings: Vec<SensorDataInput>,
        }

        let mut batch: BatchPayload = serde_json::from_slice(payload)?;

        // Asegurar device_id en todas las lecturas
        for reading in &mut batch.readings {
            reading.header.device_id = device_id.to_string();
        }

        let batch_size = batch.readings.len();
        tracing::info!(
            device_id = %device_id,
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
            device_id = %device_id,
            processed = batch_size,
            anomalies = anomalies,
            avg_quality = %avg_quality,
            "Batch procesado vía MQTT"
        );

        // Publicar respuesta
        let response_topic = format!("sensors/{}/batch_processed", device_id);
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
            tokio::spawn(async move {
                let mut cs = cloud_sync.lock().await;
                let _ = cs.sync_data(db).await;
            });
        }

        Ok(())
    }
}
