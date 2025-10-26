# IoT Gateway Edge Computing - Rust + Axum

Gateway edge computing robusto y escalable para nodos de sensores IoT (temperatura y humedad) desplegado en Raspberry Pi.

## Características Principales

### Edge Computing

- **Procesamiento Local**: Cálculos avanzados realizados en el edge antes de enviar a la nube
  - Índice de calor (Heat Index) usando fórmula NOAA
  - Punto de rocío (Dew Point) con Magnus-Tetens
  - Nivel de confort basado en temperatura y humedad
  - Detección de anomalías en tiempo real
  - Análisis de tendencias

### Arquitectura Robusta

- **Almacenamiento Local**: SQLite para persistencia resiliente
- **Sincronización Inteligente**: Envío por batches al servicio cloud principal
- **Calidad de Datos**: Evaluación automática de calidad con scoring
- **Validación**: Validación exhaustiva de datos de entrada
- **Manejo de Errores**: Sistema robusto de manejo de errores

### Escalabilidad y Performance

- **Async/Await**: Arquitectura completamente asíncrona con Tokio
- **Procesamiento Batch**: Soporte para múltiples lecturas simultáneas
- **Compresión**: Respuestas HTTP comprimidas con gzip
- **Connection Pooling**: Pool de conexiones a base de datos optimizado

## Arquitectura del Sistema

```text
┌─────────────┐         ┌──────────────────┐         ┌─────────────┐
│  ESP32      │         │   Raspberry Pi   │         │    Cloud    │
│  Sensores   │─────────│   Edge Gateway   │─────────│   Service   │
│  DHT22/BME  │  MQTT   │  ┌────────────┐  │  HTTPS  │  Principal  │
└─────────────┘         │  │ Mosquitto  │  │         └─────────────┘
                        │  │   Broker   │  │
                        │  └──────┬─────┘  │
                        │         │        │
                        │  ┌──────▼─────┐  │
                        │  │   Rust     │  │
                        │  │  Gateway   │  │
                        │  └──────┬─────┘  │
                        │         │        │
                        │  ┌──────▼─────┐  │
                        │  │  SQLite    │  │
                        │  │   Local    │  │
                        │  └────────────┘  │
                        └──────────────────┘
```

## Inicio Rápido

### Prerrequisitos

```bash
# Instalar Rust (si no lo tenemos)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# En Raspberry Pi, instalar dependencias del sistema
sudo apt update
sudo apt install build-essential pkg-config libssl-dev sqlite3 git

# Instalar Mosquitto MQTT Broker
sudo apt install mosquitto mosquitto-clients

# O usar el script incluido
sudo bash install_mosquitto.sh
```

### Instalación

#### 1. Clonar y configurar

```bash
git clone <http-repositorio>
cd env_edge_gateway_rpi

# Copiar configuración de ejemplo
cp .env.example .env

# Editar con nuestras configuraciones
nano .env
```

#### 2. Compilar

```bash
# Compilación para desarrollo
cargo build

# Compilación optimizada para producción
cargo build --release
```

#### 3. Ejecutar

```bash
# Desarrollo
cargo run

# Producción
./target/release/env_edge_gateway_rpi
```

## API Endpoints

### Protocolo Principal: MQTT

El sistema utiliza **MQTT** como protocolo principal de comunicación entre sensores y gateway.

## Configuración de Mosquitto MQTT

### Instalación Rápida

```bash
# Usar el script incluido
sudo bash install_mosquitto.sh

# O manualmente:
sudo apt install mosquitto mosquitto-clients
sudo systemctl enable mosquitto
sudo systemctl start mosquitto
```

### Configuración Básica

Archivo: `/etc/mosquitto/conf.d/env_edge_gateway_rpi.conf`

```conf
listener 1883
protocol mqtt
allow_anonymous true  # Cambiar en producción
persistence true
persistence_location /var/lib/mosquitto/
```

Con Autenticación (Producción)

```bash
# Crear usuario y contraseña
sudo mosquitto_passwd -c /etc/mosquitto/passwd ienv_edge_gateway_rpi

# Editar configuración
sudo nano /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf
```

```conf
listener 1883
allow_anonymous false
password_file /etc/mosquitto/passwd
```

### Pruebas

```bash
# Terminal 1: Suscribirse
mosquitto_sub -h localhost -t 'sensors/#' -v

# Terminal 2: Publicar
mosquitto_pub -h localhost \
  -t 'sensors/test/data' \
  -m '{"temperature": 25.5, "humidity": 65.0}'

# Ver logs
tail -f /var/log/mosquitto/mosquitto.log
```

Ver [MQTT.md](./MQTT.md) para documentación completa de MQTT.

#### Topics MQTT

**Publicar datos (ESP32 → Gateway):**

- `sensors/{sensor_id}/data` - Dato individual
- `sensors/{sensor_id}/batch` - Batch de datos

**Recibir respuestas (Gateway → ESP32):**

- `sensors/{sensor_id}/processed` - Métricas procesadas
- `sensors/{sensor_id}/batch_processed` - Respuesta de batch

**Ejemplo de publicación:**

```bash
# Publicar dato individual
mosquitto_pub -h localhost \
  -t 'sensors/esp32-001/data' \
  -m '{"temperature": 25.5, "humidity": 65.0}'

# Suscribirse a respuestas
mosquitto_sub -h localhost \
  -t 'sensors/esp32-001/processed' -v
```

### HTTP API (Monitoreo y Debug)

#### POST /api/v1/sensor/data

Recibe una lectura individual de un sensor ESP32.

**Request Body:**

```json
{
  "sensor_id": "esp32-sensor-001",
  "temperature": 25.5,
  "humidity": 65.0,
  "timestamp": "2025-10-22T10:30:00Z",
  "battery_level": 85.0,
  "rssi": -65
}
```

**Response:**

```json
{
  "status": "success",
  "message": "Datos recibidos y procesados",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "gateway_timestamp": "2025-10-22T10:30:01Z",
    "computed_metrics": {
      "heat_index": 26.2,
      "dew_point": 18.3,
      "comfort_level": 85.5,
      "is_anomaly": false
    },
    "quality_score": 95
  }
}
```

#### POST /api/v1/sensor/batch

Recibe múltiples lecturas en batch.

**Request Body:**

```json
{
  "readings": [
    {
      "sensor_id": "esp32-sensor-001",
      "temperature": 25.5,
      "humidity": 65.0
    },
    {
      "sensor_id": "esp32-sensor-002",
      "temperature": 24.8,
      "humidity": 68.0
    }
  ]
}
```

El gateway también expone endpoints HTTP para monitoreo:

#### GET /health

Health check del gateway.

```json
{
  "status": "ok",
  "gateway_id": "gateway-raspberry-pi-001",
  "version": "0.1.0",
  "timestamp": "2025-10-22T10:30:00Z",
  "components": {
    "database": "healthy",
    "edge_processor": "healthy",
    "cloud_sync": "healthy"
  },
  "metrics": {
    "pending_sync": 15
  }
}
```

#### GET /metrics

Métricas operacionales del gateway.

#### GET /api/v1/data/recent?sensor_id=XXX&limit=20

Consulta de datos recientes (útil para debugging).

#### GET /api/v1/data/stats

Estadísticas agregadas del gateway.

## Algoritmos de Edge Computing

### 1. Heat Index (Índice de Calor)

Utiliza la fórmula de Rothfusz basada en NOAA para calcular la temperatura percibida considerando humedad.

```text
HI = -42.379 + 2.04901523*T + 10.14333127*RH - 0.22475541*T*RH 
     - 0.00683783*T² - 0.05481717*RH² + 0.00122874*T²*RH 
     + 0.00085282*T*RH² - 0.00000199*T²*RH²
```

### 2. Dew Point (Punto de Rocío)

Fórmula de Magnus-Tetens para calcular la temperatura a la cual el aire se satura.

```text
α = ((a*T)/(b+T)) + ln(RH/100)
Td = (b*α)/(a-α)
```

### 3. Comfort Level (Nivel de Confort)

Algoritmo propietario que evalúa el confort humano en escala 0-100 basado en:

- Zona de confort ideal: 20-24°C y 40-60% humedad
- Penalizaciones por desviación de rangos óptimos

### 4. Anomaly Detection (Detección de Anomalías)

Sistema de detección multicapa:

- Rangos extremos fuera de valores físicos normales
- Cambios bruscos respecto a lecturas anteriores
- Patrones inconsistentes de datos

### 5. Data Quality Scoring

Evaluación de calidad con scoring 0-100 considerando:

- Nivel de batería del sensor
- Intensidad de señal WiFi (RSSI)
- Presencia de anomalías
- Valores dentro de rangos razonables

## Base de Datos Local

El gateway usa SQLite para almacenamiento resiliente con el siguiente esquema:

```sql
CREATE TABLE sensor_readings (
    id TEXT PRIMARY KEY,
    sensor_id TEXT NOT NULL,
    temperature REAL NOT NULL,
    humidity REAL NOT NULL,
    gateway_timestamp TEXT NOT NULL,
    sensor_timestamp TEXT,
    
    -- Métricas computadas
    heat_index REAL NOT NULL,
    dew_point REAL NOT NULL,
    comfort_level REAL NOT NULL,
    is_anomaly INTEGER NOT NULL,
    temperature_trend INTEGER NOT NULL,
    humidity_trend INTEGER NOT NULL,
    
    -- Calidad de datos
    quality_score INTEGER NOT NULL,
    quality_issues TEXT,
    quality_corrected INTEGER NOT NULL,
    
    -- Metadatos
    battery_level REAL,
    rssi INTEGER,
    
    -- Sincronización
    synced INTEGER NOT NULL DEFAULT 0,
    sync_attempts INTEGER NOT NULL DEFAULT 0,
    last_sync_attempt TEXT,
    
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

## Sincronización con Cloud

### Estrategia de Sincronización

1. **Por Batches**: Acumula N lecturas antes de enviar
2. **Periódica**: Sincronización cada X segundos (configurable)
3. **Resiliente**: Reintentos automáticos en caso de fallo
4. **Optimizada**: Compresión y batching para reducir ancho de banda

### Formato de Payload al Cloud

```json
{
  "gateway_id": "gateway-raspberry-pi-001",
  "gateway_version": "0.1.0",
  "sent_at": "2025-10-22T10:30:00Z",
  "batch_stats": {
    "total_readings": 50,
    "anomalies_detected": 2,
    "sensors_count": 5,
    "avg_quality_score": 92.5
  },
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "sensor_id": "esp32-sensor-001",
      "temperature": 25.5,
      "humidity": 65.0,
      "gateway_timestamp": "2025-10-22T10:29:00Z",
      "computed": {
        "heat_index": 26.2,
        "dew_point": 18.3,
        "comfort_level": 85.5,
        "is_anomaly": false
      },
      "quality": {
        "score": 95,
        "issues": []
      }
    }
  ]
}
```

## Optimizaciones para Raspberry Pi

### Compilación Optimizada

```bash
# Para Raspberry Pi 4 (64-bit)
cargo build --release --target=aarch64-unknown-linux-gnu

# Para modelos más antiguos (32-bit)
cargo build --release --target=armv7-unknown-linux-gnueabihf
```

### Systemd Service

Crear `/etc/systemd/system/env_edge_gateway_rpi.service`:

```ini
[Unit]
Description=IoT Gateway Edge Computing Service
Documentation=https://github.com/RockyCott/env_edge_gateway_rpi
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=pi
Group=pi
WorkingDirectory=/home/pi/projects/env_edge_gateway_rpi
EnvironmentFile=/home/pi/projects/env_edge_gateway_rpi/.env
ExecStart=/home/pi/projects/env_edge_gateway_rpi/target/release/env_edge_gateway_rpi

# Restart policy
Restart=always
RestartSec=10
StartLimitInterval=0

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=env_edge_gateway_rpi

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=/home/pi/projects/env_edge_gateway_rpi

[Install]
WantedBy=multi-user.target
```

Activar:

```bash
sudo systemctl daemon-reload
sudo systemctl enable env_edge_gateway_rpi
sudo systemctl start env_edge_gateway_rpi
sudo systemctl status env_edge_gateway_rpi
```

## Logging

El sistema usa `tracing` para logging estructurado:

```bash
# Ver todos los logs
RUST_LOG=debug cargo run

# Solo logs de la app
RUST_LOG=env_edge_gateway_rpi=debug cargo run

# Producción (solo errores importantes)
RUST_LOG=warn cargo run
```

## Troubleshooting

### El gateway no se conecta al cloud

- Verificar `CLOUD_SERVICE_URL` en `.env`
- Verificar `CLOUD_API_KEY`
- Revisar logs: `journalctl -u env_edge_gateway_rpi -f`

### Los sensores no envían datos

- Verificar que la Raspberry Pi sea accesible desde la red
- Verificar firewall: `sudo ufw allow 3000/tcp`
- Probar endpoint: `curl http://localhost:3000/health`

### Base de datos llena

- Ajustar `DATA_RETENTION_DAYS` en `.env`
- Limpiar manualmente: `sqlite3 sensor_data.db "DELETE FROM sensor_readings WHERE synced = 1 AND created_at < date('now', '-7 days');"`

## Estructura del Proyecto

```text
env_edge_gateway_rpi/
├── Cargo.toml              # Dependencias del proyecto
├── .env.example            # Configuración de ejemplo
├── README.md               # Este archivo
├── src/
│   ├── main.rs            # Punto de entrada
│   ├── config.rs          # Gestión de configuración
│   ├── models.rs          # Modelos de datos
│   ├── database.rs        # Capa de persistencia
│   ├── error.rs           # Manejo de errores
│   ├── handlers/          # Handlers HTTP
│   │   ├── mod.rs
│   │   ├── sensor.rs      # Ingesta de datos
│   │   ├── health.rs      # Health check
│   │   ├── metrics.rs     # Métricas
│   │   └── query.rs       # Consultas
│   └── services/          # Lógica de negocio
│       ├── mod.rs
│       ├── edge_processor.rs  # Edge computing
│       └── cloud_sync.rs      # Sincronización cloud
└── sensor_data.db         # Base de datos SQLite (generada)
```

## 🤝 Contribuciones

Las contribuciones son bienvenidas. Por favor:

1. Fork el repositorio
2. Crea una branch para la feature
3. Commit tus cambios
4. Push a la branch
5. Abre un Pull Request

## Licencia

MIT License - Ver archivo LICENSE para más detalles

## Roadmap

- [x] MQTT como alternativa a HTTP
- [ ] Machine Learning local para predicción de tendencias
- [ ] Soporte para más tipos de sensores (CO2, presión, luz)
- [ ] Dashboard web integrado
- [ ] Alertas locales por umbrales
- [ ] Backup automático de base de datos
- [ ] Métricas de Prometheus
- [ ] API GraphQL
- [ ] OTA updates para ESP32
- [ ] Clustering de múltiples gateways
- [ ] Soporte para Zigbee y Z-Wave
- [ ] Integración con Home Assistant
- [ ] Contenedorización con Docker
- [ ] Soporte para ARM64 y ARM32 adicionales
- [ ] Documentación detallada de API con OpenAPI/Swagger
- [ ] Tests unitarios y de integración exhaustivos
- [ ] Internacionalización (i18n) y soporte multilenguaje
- [ ] Optimización avanzada de consumo energético
