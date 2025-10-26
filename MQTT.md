# Arquitectura MQTT del IoT Gateway

El sistema utiliza **MQTT (Message Queuing Telemetry Transport)** como protocolo de comunicación entre los sensores ESP32 y el gateway en Raspberry Pi. MQTT es ideal para IoT por ser ligero, eficiente en ancho de banda y soportar diferentes niveles de QoS.

## Arquitectura MQTT

```text
┌─────────────────────────────────────────────────────────────────┐
│                    Arquitectura MQTT                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐         ┌──────────────────┐         ┌─────────┐ │
│  │  ESP32   │         │   Raspberry Pi   │         │  Cloud  │ │
│  │ Sensor 1 │────┐    │                  │         │ Service │ │
│  └──────────┘    │    │  ┌────────────┐  │         └─────────┘ │
│                  │    │  │ Mosquitto  │  │              ▲       │
│  ┌──────────┐    ├───▶│  │   Broker   │  │              │       │
│  │  ESP32   │    │    │  └─────┬──────┘  │              │       │
│  │ Sensor 2 │────┤    │        │         │              │       │
│  └──────────┘    │    │        ▼         │              │       │
│                  │    │  ┌────────────┐  │              │       │
│  ┌──────────┐    │    │  │   Rust     │  │              │       │
│  │  ESP32   │    └───▶│  │  Gateway   │──┼──────────────┘       │
│  │ Sensor N │         │  │  Handler   │  │    HTTPS              │
│  └──────────┘         │  └────────────┘  │                      │
│                       │                  │                      │
│     MQTT Pub          │   MQTT Sub       │                      │
│   (WiFi/LAN)          │   (localhost)    │                      │
│                       └──────────────────┘                      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Topics MQTT

### Estructura de Topics

El sistema utiliza una estructura jerárquica de topics:

```text
sensors/
├── {sensor_id}/
│   ├── data              # Publicación de datos individuales
│   ├── batch             # Publicación de batches
│   ├── processed         # Respuesta con métricas procesadas
│   ├── batch_processed   # Respuesta de batch procesado
│   └── status            # Estado del sensor (futuro)
```

### Topics de Publicación (ESP32 → Gateway)

#### 1. `sensors/{sensor_id}/data`

Publicación de una lectura individual.

**QoS**: 1 (At Least Once)

**Payload**:

```json
{
  "temperature": 25.5,
  "humidity": 65.0,
  "battery_level": 85.0,
  "rssi": -65
}
```

#### 2. `sensors/{sensor_id}/batch`

Publicación de múltiples lecturas.

**QoS**: 1 (At Least Once)

**Payload**:

```json
{
  "readings": [
    {
      "temperature": 25.5,
      "humidity": 65.0,
      "battery_level": 85.0,
      "rssi": -65
    },
    {
      "temperature": 26.0,
      "humidity": 67.0,
      "battery_level": 84.0,
      "rssi": -66
    }
  ]
}
```

### Topics de Respuesta (Gateway → ESP32)

#### 3. `sensors/{sensor_id}/processed`

Respuesta con métricas procesadas para un dato individual.

**QoS**: 0 (At Most Once) - No crítico

**Payload**:

```json
{
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
```

#### 4. `sensors/{sensor_id}/batch_processed`

Respuesta de batch procesado.

**QoS**: 0 (At Most Once)

**Payload**:

```json
{
  "status": "success",
  "processed_count": 5,
  "anomalies_detected": 1,
  "average_quality_score": 92.5
}
```

## Flujo de Datos MQTT

### Flujo Completo

```text
┌─────────────────────────────────────────────────────────────────┐
│                    Flujo de Datos MQTT                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ESP32                  Mosquitto              Rust Gateway      │
│  Sensor                  Broker                                  │
│    │                       │                        │            │
│    │ 1. Connect MQTT       │                        │            │
│    ├──────────────────────▶│                        │            │
│    │                       │                        │            │
│    │ 2. CONNACK            │                        │            │
│    │◀──────────────────────┤                        │            │
│    │                       │  3. Subscribe          │            │
│    │                       │  sensors/+/data        │            │
│    │                       │◀───────────────────────┤            │
│    │                       │                        │            │
│    │ 4. Publish            │                        │            │
│    │ sensors/esp32-1/data  │                        │            │
│    ├──────────────────────▶│                        │            │
│    │                       │                        │            │
│    │                       │  5. Forward Message    │            │
│    │                       ├───────────────────────▶│            │
│    │                       │                        │            │
│    │                       │                        │ 6. Process │
│    │                       │                        │ - Validate │
│    │                       │                        │ - Compute  │
│    │                       │                        │ - Store    │
│    │                       │                        │            │
│    │                       │  7. Publish Response   │            │
│    │                       │ sensors/esp32-1/       │            │
│    │                       │      processed         │            │
│    │                       │◀───────────────────────┤            │
│    │ 8. Receive Response   │                        │            │
│    │◀──────────────────────┤                        │            │
│    │                       │                        │            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Configuración Mosquitto

### Archivo de Configuración Básico

**Ubicación**: `/etc/mosquitto/conf.d/env_edge_gateway_rpi.conf`

```conf
# Listener MQTT estándar
listener 1883
protocol mqtt

# Logging
# log_dest file /var/log/mosquitto/mosquitto.log
log_dest stdout
log_type all

# Persistencia
persistence true
# persistence_location /var/lib/mosquitto/

# Configuración de mensajes
message_size_limit 1048576
max_keepalive 60
max_qos 2

# Sin autenticación (desarrollo)
allow_anonymous true
```

### Configuración con Autenticación (Producción)

```conf
# Listener MQTT
listener 1883
protocol mqtt

# Autenticación
allow_anonymous false
password_file /etc/mosquitto/passwd

# ACL (Access Control List)
acl_file /etc/mosquitto/acl

# Logging
# log_dest file /var/log/mosquitto/mosquitto.log
log_type error
log_type warning
log_type notice

# Persistencia
persistence true
# persistence_location /var/lib/mosquitto/

# Límites
message_size_limit 1048576
max_connections 100
max_keepalive 60
```

### Crear Usuario y Contraseña

```bash
# Crear usuario
sudo mosquitto_passwd -c /etc/mosquitto/passwd env_edge_gateway_rpi

# Agregar más usuarios
sudo mosquitto_passwd /etc/mosquitto/passwd esp32_sensor_001

# Reiniciar Mosquitto
sudo systemctl restart mosquitto
```

### ACL (Control de Acceso)

**Archivo**: `/etc/mosquitto/acl`

```conf
# Gateway puede leer y escribir todo
user env_edge_gateway_rpi
topic readwrite #

# Cada sensor solo puede publicar en su topic
user esp32_sensor_001
topic write sensors/esp32-sensor-001/#
topic read sensors/esp32-sensor-001/processed
topic read sensors/esp32-sensor-001/batch_processed

user esp32_sensor_002
topic write sensors/esp32-sensor-002/#
topic read sensors/esp32-sensor-002/processed
topic read sensors/esp32-sensor-002/batch_processed
```

## Ventajas de MQTT vs HTTP

### Comparación

| Característica | MQTT | HTTP (REST) |
|---------------|------|-------------|
| **Overhead** | Muy bajo (~2 bytes header) | Alto (~100+ bytes) |
| **Conexión** | Persistente | Request/Response |
| **Bidireccional** | Nativo | Polling necesario |
| **QoS** | 0, 1, 2 | No nativo |
| **Retención** | Mensajes retenidos | No |
| **Batería** | Muy eficiente | Menos eficiente |
| **Latencia** | Muy baja | Mayor |
| **Ancho de banda** | Mínimo | Mayor |

### Casos de Uso

**Usar MQTT cuando**:

- Sensores con batería limitada
- Red inestable (QoS ayuda)
- Se necesita baja latencia
- Muchos sensores simultáneos
- Comunicación bidireccional

**Usa HTTP cuando**:

- Integraciones web directas
- Operaciones CRUD específicas
- Firewall restrictivos (solo 80/443)
- No hay broker disponible

## Testing del Sistema MQTT

### Pruebas con Cliente MQTT

```bash
# Terminal 1: Suscribirse a todos los topics
mosquitto_sub -h localhost -t 'sensors/#' -v

# Terminal 2: Publicar dato individual
mosquitto_pub -h localhost \
  -t 'sensors/test-sensor/data' \
  -m '{"temperature": 25.5, "humidity": 65.0}'

# Terminal 3: Publicar batch
mosquitto_pub -h localhost \
  -t 'sensors/test-sensor/batch' \
  -m '{"readings": [{"temperature": 25.5, "humidity": 65.0}]}'
```

### Script de Prueba Python

```python
import paho.mqtt.client as mqtt
import json
import time

def on_connect(client, userdata, flags, rc):
    print(f"Conectado con código: {rc}")
    client.subscribe("sensors/test-sensor/processed")

def on_message(client, userdata, msg):
    print(f"Respuesta recibida: {msg.topic}")
    print(json.dumps(json.loads(msg.payload), indent=2))

client = mqtt.Client()
client.on_connect = on_connect
client.on_message = on_message

client.connect("localhost", 1883, 60)
client.loop_start()

# Publicar datos de prueba
data = {
    "temperature": 25.5,
    "humidity": 65.0,
    "battery_level": 85.0,
    "rssi": -65
}

client.publish("sensors/test-sensor/data", json.dumps(data))

time.sleep(5)
client.loop_stop()
```

## Monitoreo de Mosquitto

### Ver Logs en Tiempo Real

```bash
# Ver logs del sistema
sudo journalctl -u mosquitto -f

# Ver logs del archivo
tail -f /var/log/mosquitto/mosquitto.log
```

### Estadísticas del Broker

```bash
# Suscribirse a estadísticas del broker
mosquitto_sub -h localhost -t '$SYS/#' -v

# Estadísticas específicas
mosquitto_sub -h localhost -t '$SYS/broker/clients/connected'
mosquitto_sub -h localhost -t '$SYS/broker/messages/received'
mosquitto_sub -h localhost -t '$SYS/broker/messages/sent'
```

### 3. Buffer de Mensajes

```conf
# Aumentar si hay muchos sensores
max_queued_messages 1000
max_inflight_messages 20
```

## Futuras Mejoras

- [ ] Bridge MQTT entre múltiples gateways
- [ ] MQTT over WebSockets para dashboard web
- [ ] Compresión de payloads
- [ ] Last Will and Testament (LWT) para detección de desconexión
- [ ] Retained messages para estado de sensores
