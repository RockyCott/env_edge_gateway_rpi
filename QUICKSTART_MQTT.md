# Quick Start Guide - IoT Gateway con MQTT

## Requisitos

- Raspberry Pi 3B+ o superior (recomendado: Pi 4 con 4GB RAM)
- Tarjeta microSD con Raspberry Pi OS instalado
- Conexión a internet
- 1 o más ESP32 con sensores DHT22/BME280

## ⚡ Instalación Rápida (3 Pasos)

### Paso 1: Preparar Raspberry Pi

```bash
# SSH a tu Raspberry Pi
ssh pi@raspberrypi.local

# Actualizar sistema
sudo apt update && sudo apt upgrade -y

# Instalar dependencias
sudo apt install -y build-essential pkg-config libssl-dev sqlite3 \
    mosquitto mosquitto-clients git curl

# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verificar instalaciones
rustc --version
mosquitto -h
```

### Paso 2: Configurar Mosquitto MQTT

```bash
# Configurar Mosquitto
sudo nano /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf
```

Agregar:

```conf
listener 1883
protocol mqtt
allow_anonymous true
persistence true
# persistence_location /var/lib/mosquitto/
# log_dest file /var/log/mosquitto/mosquitto.log
```

```bash
# Reiniciar Mosquitto
sudo systemctl restart mosquitto
sudo systemctl enable mosquitto

# Probar
mosquitto_pub -h localhost -t test -m "Hello MQTT"
mosquitto_sub -h localhost -t test -C 1
```

### Paso 3: Compilar y Ejecutar Gateway

```bash
# Clonar proyecto
mkdir -p ~/projects && cd ~/projects
git clone <tu-repo> env_edge_gateway_rpi
cd env_edge_gateway_rpi

# Configurar
cp .env.example .env
nano .env
```

Editar `.env`:

```bash
GATEWAY_ID=gateway-rpi-001
DATABASE_URL=sqlite://sensor_data.db
CLOUD_SERVICE_URL=https://cloud-service.com/api/v1/ingest
CLOUD_API_KEY=api_key

MQTT_BROKER_HOST=localhost
MQTT_BROKER_PORT=1883
MQTT_CLIENT_ID=gateway-rpi-mqtt-001

CLOUD_SYNC_BATCH_SIZE=50
CLOUD_SYNC_INTERVAL_SECS=300
RUST_LOG=info
```

```bash
# Compilar
cargo build --release

# Ejecutar
./target/release/env_edge_gateway_rpi
```

## Verificación

### 1. Verificar Mosquitto

```bash
# En Raspberry Pi
sudo systemctl status mosquitto
# Debe mostrar: active (running)

# Ver logs
tail -f /var/log/mosquitto/mosquitto.log
```

### 2. Verificar Gateway

```bash
# Ver logs del gateway
# (si lo ejecutas manualmente, verás logs en terminal)
# Si está como servicio:
sudo journalctl -u env_edge_gateway_rpi -f
```

Se debería ver:

```text
Iniciando IoT Gateway Edge Computing con MQTT
Configuración cargada
Base de datos inicializada
Servicios de edge computing inicializados
Conectando a broker MQTT
Suscrito a topics: sensors/+/data, sensors/+/batch
MQTT Handler iniciado, escuchando mensajes...
```

### 3. Monitorear en Tiempo Real

```bash
# Terminal 1: Ver todos los mensajes MQTT
mosquitto_sub -h localhost -t 'sensors/#' -v

# Terminal 2: Ver logs del gateway
sudo journalctl -u env_edge_gateway_rpi -f

# Terminal 3: Consultar base de datos
sqlite3 ~/projects/env_edge_gateway_rpi/sensor_data.db \
    "SELECT sensor_id, temperature, humidity, gateway_timestamp FROM sensor_readings ORDER BY gateway_timestamp DESC LIMIT 5;"
```

## Pruebas Manuales

### Publicar Dato de Prueba

```bash
# Publicar dato
mosquitto_pub -h localhost \
    -t 'sensors/manual-test/data' \
    -m '{"temperature": 25.5, "humidity": 65.0, "battery_level": 85.0, "rssi": -65}'

# Esperar respuesta (en otra terminal)
mosquitto_sub -h localhost \
    -t 'sensors/manual-test/processed' -v
```

### Ejecutar Script de Pruebas

```bash
chmod +x test_mqtt.sh
./test_mqtt.sh localhost 1883
```

## Configurar como Servicio (Opcional)

```bash
# Crear servicio systemd
sudo nano /etc/systemd/system/env_edge_gateway_rpi.service
```

```ini
[Unit]
Description=IoT Gateway Edge Computing MQTT
After=network.target mosquitto.service
Requires=mosquitto.service

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi/projects/env_edge_gateway_rpi
EnvironmentFile=/home/pi/projects/env_edge_gateway_rpi/.env
ExecStart=/home/pi/projects/env_edge_gateway_rpi/target/release/env_edge_gateway_rpi
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
# Habilitar y arrancar
sudo systemctl daemon-reload
sudo systemctl enable env_edge_gateway_rpi
sudo systemctl start env_edge_gateway_rpi
sudo systemctl status env_edge_gateway_rpi
```

## Dashboard de Monitoreo

### Ver Health Check

```bash
curl http://localhost:2883/health | jq
```

Respuesta:

```json
{
  "status": "ok",
  "gateway_id": "gateway-rpi-001",
  "version": "0.1.0",
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

### Ver Métricas

```bash
curl http://localhost:2883/metrics | jq
```

### Ver Datos Recientes

```bash
curl "http://localhost:2883/api/v1/data/recent?sensor_id=esp32-sensor-001&limit=5" | jq
```

## Seguridad (Producción)

### 1. Habilitar Autenticación MQTT

```bash
# Crear usuario
sudo mosquitto_passwd -c /etc/mosquitto/passwd env_edge_gateway_rpi

# Editar configuración
sudo nano /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf
```

Cambiar:

```conf
allow_anonymous false
password_file /etc/mosquitto/passwd
```

Actualizar `.env`:

```bash
MQTT_USERNAME=env_edge_gateway_rpi
MQTT_PASSWORD=tu_password_segura
```

### 2. Configurar Firewall

```bash
sudo apt install ufw
sudo ufw allow 22/tcp    # SSH
sudo ufw allow 1883/tcp  # MQTT
sudo ufw allow 2883/tcp  # HTTP Gateway
sudo ufw enable
```

## Conectar Múltiples Sensores

Para cada sensor adicional:

1. Cargar el código en ESP32
2. Cambiar `SENSOR_ID` a un valor único:

   ```rust
   String SENSOR_ID = "esp32-sensor-002";  // Incrementar número
   ```

3. El gateway automáticamente procesará datos de todos los sensores

## Troubleshooting Rápido

### Gateway no inicia

```bash
# Ver error específico
./target/release/env_edge_gateway_rpi

# Verificar variables de entorno
cat .env
```

### ESP32 no conecta a MQTT

```bash
# Verificar Mosquitto está corriendo
sudo systemctl status mosquitto

# Verificar puerto está abierto
netstat -tulpn | grep 1883

# Probar conexión desde ESP32 IP
mosquitto_pub -h 192.168.1.100 -t test -m "hello"
```

### No se reciben mensajes

```bash
# Monitorear topics
mosquitto_sub -h localhost -t '#' -v

# Ver logs del gateway
sudo journalctl -u env_edge_gateway_rpi -f

# Ver logs de Mosquitto
tail -f /var/log/mosquitto/mosquitto.log
```

### Base de datos crece mucho

```bash
# Limpiar datos antiguos
sqlite3 sensor_data.db \
    "DELETE FROM sensor_readings WHERE synced=1 AND datetime(gateway_timestamp) < datetime('now', '-7 days');"

# Vacuum
sqlite3 sensor_data.db "VACUUM;"
```

¡Listo! el sistema IoT Gateway está funcionando. Los datos fluyen:

```text
ESP32 → MQTT (Mosquitto) → Gateway (Rust) → SQLite → Cloud
```

**Próximos pasos:**

- Agregar más sensores ESP32
- Configurar el servicio cloud
- Implementar alertas
- Crear dashboards de visualización

## Recursos Adicionales

- [README.md](./README.md) - Documentación completa
- [MQTT.md](./MQTT.md) - Arquitectura MQTT detallada
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Diseño del sistema
- [DEPLOYMENT.md](./DEPLOYMENT.md) - Guía de despliegue completa

## Soporte

Si encuentra problemas:

1. Revisa los logs: `sudo journalctl -u env_edge_gateway_rpi -f`
2. Verifica conectividad: `./test_mqtt.sh`
3. Consulta la documentación completa
4. Abre un issue en el repositorio
