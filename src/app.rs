use std::sync::Arc;
use tokio::{net::TcpListener, sync::Mutex};
use tracing::info;

use crate::{
    config::Config,
    database::Database,
    services::{cloud_sync::CloudSync, edge_processor::EdgeProcessor, mqtt_handler::MqttHandler},
    startup::{logger, router::build_router, state::AppState},
};

pub async fn bootstrap() -> anyhow::Result<()> {
    // Inicializar logger
    logger::init();
    info!("Iniciando IoT Gateway Edge Computing...");

    // Cargar configuración
    let config = Arc::new(Config::load()?);
    info!("Configuración cargada correctamente");

    // Base de datos
    let db = Database::new(&config.database_url).await?;
    db.migrate().await?;
    info!("Base de datos SQLite inicializada");

    // Inicializar servicios
    let edge_processor = Arc::new(EdgeProcessor::new(config.clone()));
    let cloud_sync = Arc::new(Mutex::new(CloudSync::new(config.clone())));

    // Lanzar tareas en background
    let db_clone = db.clone();
    let cloud_sync_clone = cloud_sync.clone();
    tokio::spawn(async move {
        let mut cs = cloud_sync_clone.lock().await;
        cs.start_sync_task(db_clone).await;
    });

    info!("Servicios de edge computing listos");

    // Iniciar MQTT handler
    let mqtt_handler = MqttHandler::new(
        config.clone(),
        db.clone(),
        edge_processor.clone(),
        cloud_sync.clone(),
    )
    .await?;
    let mqtt_task = mqtt_handler.start().await;

    // Crear estado compartido
    let state = AppState {
        db,
        edge_processor,
        cloud_sync,
        config: config.clone(),
    };

    // Construir el router
    let app = build_router(state);

    // Servidor HTTP
    let addr = format!("0.0.0.0:{}", config.http_port.unwrap_or(3000));
    let listener = TcpListener::bind(&addr).await?;
    info!("Servidor HTTP escuchando en {}", addr);
    info!(
        "Broker MQTT: {}:{}",
        config.mqtt_broker_host, config.mqtt_broker_port
    );

    // Ejecutar servidor + MQTT handler concurrentemente
    let http_server = axum::serve(listener, app);
    tokio::select! {
        result = http_server => {
            if let Err(e) = result {
                tracing::error!("Error en el servidor HTTP: {}", e);
            }
        }
        _ = mqtt_task => {
            tracing::error!("MQTT Handler ha finalizado inesperadamente");
        }
    }

    Ok(())
}
