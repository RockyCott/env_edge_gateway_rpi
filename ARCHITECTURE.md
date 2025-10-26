# Arquitectura del Sistema IoT Gateway

Este documento describe la arquitectura técnica del gateway edge computing para sensores IoT desarrollado en Rust con Axum.

## Componentes Principales

### 1. Capa de Presentación (HTTP API)

**Tecnologías**: Axum, Tower, Tower-HTTP

La capa de presentación expone endpoints RESTful para:

- Ingesta de datos desde sensores ESP32
- Health checks y monitoreo
- Consultas de datos locales

**Middlewares aplicados**:

- `TraceLayer`: Logging de requests/responses
- `CorsLayer`: Manejo de CORS para acceso desde diferentes orígenes
- `CompressionLayer`: Compresión gzip de respuestas

### 2. Capa de Lógica de Negocio

#### 2.1 Edge Processor (`services/edge_processor.rs`)

Implementa los algoritmos de edge computing:

```text
Datos Crudos → Validación → Procesamiento → Métricas Derivadas
                                ↓
                          Detección de Anomalías
                                ↓
                          Evaluación de Calidad
```

**Algoritmos implementados**:

1. **Heat Index** (Índice de Calor)
   - Entrada: Temperatura (°C), Humedad Relativa (%)
   - Salida: Temperatura percibida (°C)
   - Uso: Evaluación de riesgo de golpe de calor

2. **Dew Point** (Punto de Rocío)
   - Entrada: Temperatura (°C), Humedad Relativa (%)
   - Salida: Temperatura de punto de rocío (°C)
   - Uso: Predicción de condensación

3. **Comfort Level** (Nivel de Confort)
   - Entrada: Temperatura (°C), Humedad Relativa (%)
   - Salida: Score 0-100
   - Uso: Evaluación de condiciones ambientales para confort humano

4. **Anomaly Detection** (Detección de Anomalías)
   - Entrada: Todas las métricas del sensor
   - Salida: Boolean (anomalía detectada)
   - Uso: Identificación de lecturas erróneas o condiciones extremas

5. **Data Quality Scoring** (Puntuación de Calidad)
   - Entrada: Datos del sensor + metadatos
   - Salida: Score 0-100 + lista de issues
   - Factores: Batería, señal WiFi, anomalías, rangos

#### 2.2 Cloud Sync (`services/cloud_sync.rs`)

Gestiona la sincronización con el servicio cloud:

```text
┌─────────────────────────────────────────────┐
│         Estrategia de Sincronización        │
├─────────────────────────────────────────────┤
│                                             │
│  Trigger 1: Batch Size Threshold            │
│  ├─ Si pending_count >= BATCH_SIZE          │
│  └─ Enviar inmediatamente                   │
│                                             │
│  Trigger 2: Timer Periódico                 │
│  ├─ Cada SYNC_INTERVAL_SECS                 │
│  └─ Enviar datos pendientes                 │
│                                             │
│  Manejo de Errores:                         │
│  ├─ Retry con exponential backoff           │
│  ├─ Datos permanecen locales si falla       │
│  └─ Logging detallado de errores            │
│                                             │
└─────────────────────────────────────────────┘
```

**Características**:

- Batching inteligente para reducir requests HTTP
- Compresión de payload
- Autenticación con Bearer tokens
- Resiliencia ante fallos de red
- Estadísticas agregadas en cada batch

### 3. Capa de Persistencia

#### 3.1 Database (`database.rs`)

**Tecnología**: SQLite con SQLx

**¿Por qué SQLite?**

- Embedded: No requiere servidor separado
- Ligero: Ideal para Raspberry Pi
- ACID compliant: Garantiza integridad
- File-based: Fácil backup y migración

**Esquema de Base de Datos**:

```sql
sensor_readings
├── id (PK)                    -- UUID único
├── sensor_id                  -- Identificador del sensor
├── temperature                -- Temperatura en °C
├── humidity                   -- Humedad relativa %
├── gateway_timestamp          -- Timestamp del gateway
├── sensor_timestamp           -- Timestamp del sensor (opcional)
│
├── [Métricas Computadas]
├── heat_index                 -- Índice de calor calculado
├── dew_point                  -- Punto de rocío
├── comfort_level              -- Nivel de confort (0-100)
├── is_anomaly                 -- Anomalía detectada (bool)
├── temperature_trend          -- Tendencia temp (-1,0,1)
├── humidity_trend             -- Tendencia humedad (-1,0,1)
│
├── [Calidad de Datos]
├── quality_score              -- Score 0-100
├── quality_issues             -- JSON con problemas
├── quality_corrected          -- Si se corrigieron datos
│
├── [Metadatos del Sensor]
├── battery_level              -- Nivel de batería %
├── rssi                       -- Señal WiFi dBm
│
├── [Control de Sincronización]
├── synced                     -- Ya sincronizado (bool)
├── sync_attempts              -- Intentos de sync
├── last_sync_attempt          -- Último intento
│
└── created_at                 -- Timestamp de creación
```

**Índices para Performance**:

- `idx_sensor_id`: Consultas por sensor
- `idx_synced`: Búsqueda de datos pendientes
- `idx_gateway_timestamp`: Consultas temporales

### 4. Capa de Configuración

#### 4.1 Config (`config.rs`)

Gestión centralizada de configuración mediante variables de entorno.

**Configuraciones críticas**:

```rust
GATEWAY_ID                  // Identificador único del gateway
DATABASE_URL                // Path a SQLite
CLOUD_SERVICE_URL           // URL del servicio cloud
CLOUD_API_KEY               // Autenticación
CLOUD_SYNC_BATCH_SIZE       // Tamaño de batch (default: 50)
CLOUD_SYNC_INTERVAL_SECS    // Intervalo sync (default: 300s)
DATA_RETENTION_DAYS         // Retención local (default: 7d)
```

### 5. Manejo de Errores

#### 5.1 Error (`error.rs`)

Sistema de errores tipado usando `thiserror`:

```rust
AppError
├── ValidationError          // Datos de entrada inválidos
├── DatabaseError            // Errores de SQLite
├── SerializationError       // JSON parsing
├── InternalError            // Errores generales
├── NotFound                 // Recurso no encontrado
└── ConfigError              // Configuración incorrecta
```

Cada error se convierte automáticamente en una respuesta HTTP apropiada.

## Flujo de Datos

### Flujo Completo de Ingesta

```text
┌──────────┐     ┌────────────────────────────────────┐     ┌──────────┐
│          │     │        Raspberry Pi Gateway        │     │          │
│  ESP32   │────▶│                                    │────▶│  Cloud   │
│  Sensor  │HTTP │  1. Recepción HTTP                 │HTTPS│ Service  │
│          │     │  2. Validación (validator)         │     │          │
└──────────┘     │  3. Edge Processing                │     └──────────┘
                 │     ├─ Heat Index                  │
    POST /       │     ├─ Dew Point                   │     POST /
    sensor/data  │     ├─ Comfort Level               │     ingest
                 │     ├─ Anomaly Detection           │
    {            │     └─ Quality Scoring             │     {
      sensor_id, │  4. Almacenamiento SQLite           │       gateway_id,
      temp,      │  5. Check Sync Threshold            │       data: [...],
      humidity   │  6. Trigger Sync (si aplica)        │       stats
    }            │                                    │     }
                 └────────────────────────────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │   SQLite Local   │
                    │                  │
                    │  • Datos crudos  │
                    │  • Métricas      │
                    │  • Estado sync   │
                    └──────────────────┘
```

### Flujo de Sincronización

```text
┌─────────────────────────────────────────────────────┐
│          Tarea Periódica (Tokio Task)               │
│                                                     │
│  loop {                                             │
│    ┌─────────────────────────────────────────┐    │
│    │  1. Obtener datos pendientes (synced=0) │    │
│    └────────────────┬────────────────────────┘    │
│                     │                              │
│    ┌────────────────▼─────────────────────────┐   │
│    │  2. Agregar estadísticas del batch       │   │
│    │     - Total lecturas                     │   │
│    │     - Anomalías detectadas               │   │
│    │     - Sensores únicos                    │   │
│    │     - Calidad promedio                   │   │
│    └────────────────┬─────────────────────────┘   │
│                     │                              │
│    ┌────────────────▼─────────────────────────┐   │
│    │  3. Construir payload JSON               │   │
│    │     + Gateway metadata                   │   │
│    │     + Timestamp de envío                 │   │
│    └────────────────┬─────────────────────────┘   │
│                     │                              │
│    ┌────────────────▼─────────────────────────┐   │
│    │  4. POST al Cloud Service                │   │
│    │     + Bearer token auth                  │   │
│    │     + Retry en caso de error             │   │
│    └────────────────┬─────────────────────────┘   │
│                     │                              │
│         ┌───────────┴───────────┐                 │
│         │ Success?              │                 │
│         └───┬───────────────┬───┘                 │
│             │ Sí            │ No                   │
│    ┌────────▼────────┐ ┌────▼──────────────┐     │
│    │ 5. Marcar como  │ │ 5. Mantener       │     │
│    │    synced=1     │ │    synced=0       │     │
│    │    en SQLite    │ │    + Log error    │     │
│    └─────────────────┘ └───────────────────┘     │
│                                                     │
│    sleep(SYNC_INTERVAL_SECS)                      │
│  }                                                  │
└─────────────────────────────────────────────────────┘
```

## Decisiones de Diseño

### 1. Rust como Lenguaje

**Ventajas**:

- **Performance**: Cercano a C/C++, ideal para edge devices
- **Seguridad de Memoria**: Sin garbage collector, sin data races
- **Concurrencia**: Sistema de ownership previene bugs
- **Tamaño de Binario**: Pequeño y optimizable
- **Ecosistema**: Crates maduros (Tokio, Axum, SQLx)

**Trade-offs**:

- Curva de aprendizaje más pronunciada
- Tiempos de compilación más largos

**Por qué Axum**:

- Basado en Tokio (mismo runtime que SQLx)
- Tower middleware ecosystem maduro
- API ergonómica con extractors
- Excelente performance
- Mantenido por el equipo de Tokio

**Por qué SQLite**:

- No requiere proceso separado (crítico en Pi)
- Bajo consumo de RAM
- ACID compliance para integridad
- File-based: fácil backup/restore
- Suficiente para throughput esperado (< 1000 req/s)

**Limitaciones aceptadas**:

- No soporta múltiples writers concurrentes
- No distribuido (suficiente para single gateway)

**Por qué Batching**:

- Reduce overhead de HTTP
- Mejor uso de ancho de banda
- Permite compresión eficiente
- Facilita rate limiting en cloud

**Trade-off**:

- Latencia adicional (máx SYNC_INTERVAL_SECS)
- Aceptable para monitoreo ambiental (no crítico)

## Consideraciones de Seguridad

### 1. Autenticación y Autorización

```text
ESP32 ──▶ Gateway: Sin auth (red local confiable)
Gateway ──▶ Cloud: Bearer token (HTTPS)
```

**Recomendaciones para producción**:

- Agregar API keys para ESP32
- Implementar rate limiting
- Certificados TLS para ESP32-Gateway

### 2. Validación de Datos

Múltiples capas:

1. **Validación de tipo** (Serde deserialization)
2. **Validación de rango** (validator crate)
3. **Validación de negocio** (Edge Processor)

### 3. Seguridad de Datos en Reposo

- Limpieza automática de datos antiguos

## Performance y Escalabilidad

### Throughput Esperado

**Capacidad Real Medida**:

- Axum puede manejar >10,000 req/s
- SQLite: ~2,000 writes/s (sin índices)
- Bottleneck: Network I/O al cloud

### Optimizaciones Implementadas

1. **Connection Pooling**: SQLx pool para reutilizar conexiones
2. **Async I/O**: Todo non-blocking con Tokio
3. **Batch Processing**: Reduce syscalls y contención
4. **Índices DB**: Para consultas frecuentes
5. **Compresión**: gzip en respuestas HTTP

### Escalabilidad Vertical

Para más sensores en mismo gateway:

- Aumentar `CLOUD_SYNC_BATCH_SIZE`
- Ajustar pool de conexiones SQLite
- Considerar particionamiento de DB por sensor

### Escalabilidad Horizontal

Para múltiples gateways:

- Cada gateway es independiente
- Cloud service debe agregar por `gateway_id`
- Considerar load balancer si >100 gateways

## Testing Strategy

### Niveles de Testing

```text
┌────────────────────────────────────────┐
│         Pirámide de Tests              │
├────────────────────────────────────────┤
│                                        │
│           ┌──────────┐                 │
│           │   E2E    │  (5%)           │
│           │  Tests   │                 │
│           └──────────┘                 │
│         ┌──────────────┐               │
│         │ Integration  │  (20%)        │
│         │    Tests     │               │
│         └──────────────┘               │
│    ┌────────────────────────┐          │
│    │     Unit Tests         │  (75%)   │
│    └────────────────────────┘          │
│                                        │
└────────────────────────────────────────┘
```

**Unit Tests**: Algoritmos edge computing, validaciones
**Integration Tests**: Handlers + Database
**E2E Tests**: Script bash completo

## Monitoreo y Observabilidad

### Logging

Structured logging con `tracing`:

```rust
tracing::info!(
    sensor_id = %payload.sensor_id,
    temperature = %payload.temperature,
    "📡 Recibiendo datos de sensor"
);
```

**Niveles**:

- `ERROR`: Errores críticos que requieren atención
- `WARN`: Anomalías, problemas no críticos
- `INFO`: Operaciones importantes (ingesta, sync)
- `DEBUG`: Detalles de operación
- `TRACE`: Debugging profundo

### Métricas

Expuestas via `/metrics`:

- Lecturas pendientes de sync
- Configuración actual
- Estado de componentes

**Extensible a Prometheus**:

```rust
// Futuro: usar prometheus crate
gauge!("iot_gateway_pending_sync", pending as f64);
counter!("iot_gateway_readings_received", 1);
```

### Health Checks

`/health` endpoint con estados de:

- Database connectivity
- Edge processor
- Cloud sync service

## Mantenimiento y Operaciones

### Limpieza Automática

```rust
// Ejecutar periódicamente
db.cleanup_old_synced(DATA_RETENTION_DAYS).await
```

### Backup Strategy

```bash
# Backup en caliente de SQLite
sqlite3 sensor_data.db ".backup backup.db"

# Backup periódico con cron
0 2 * * * sqlite3 /app/sensor_data.db ".backup /backups/sensor-$(date +\%Y\%m\%d).db"
```

### Actualizaciones

1. **Rolling update**: Compilar nuevo binario
2. **Graceful shutdown**: Completar syncs pendientes
3. **Restart service**: systemd restart
4. **Verify**: Health check

## Roadmap Técnico

### Fase 1: Estabilización (Actual)

- [x] Core functionality
- [x] Edge computing algorithms
- [x] SQLite persistence
- [] Cloud sync

### Fase 2: Producción

- [ ] TLS/HTTPS para ESP32
- [ ] Rate limiting
- [ ] Prometheus metrics
- [ ] Distributed tracing

### Fase 3: Escalabilidad

- [ ] Multi-gateway orchestration
- [ ] ML models locales (ONNX Runtime)
- [ ] Time-series optimizations
- [ ] InfluxDB integration

### Fase 4: Features Avanzadas

- [ ] MQTT support
- [ ] Edge analytics dashboard
- [ ] Alertas locales
- [ ] OTA updates para sensores

## Referencias

- [Axum Documentation](https://docs.rs/axum)
- [SQLx Guide](https://docs.rs/sqlx)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Heat Index Formula - NOAA](https://www.wpc.ncep.noaa.gov/html/heatindex_equation.shtml)
- [Magnus-Tetens Formula](https://en.wikipedia.org/wiki/Dew_point)
