# Guía de Despliegue - IoT Gateway en Raspberry Pi

Esta guía detalla paso a paso cómo desplegar el gateway IoT en una Raspberry Pi.

## Requisitos Previos

### Hardware

- Raspberry Pi 3B+ o superior (recomendado: Pi 4 con 4GB RAM)
- Tarjeta microSD (mínimo 16GB, clase 10)
- Fuente de alimentación (5V 3A para Pi 4)
- Conexión a internet (Ethernet o WiFi)
- (Opcional) Carcasa con ventilación

### Software

- Raspberry Pi OS Lite (64-bit recomendado)
- Acceso SSH habilitado

## Paso 1: Preparar Raspberry Pi

### 1.1 Instalar Raspberry Pi OS

```bash
# Descargar Raspberry Pi Imager
# https://www.raspberrypi.com/software/

# Flashear la imagen a la microSD
# Configurar SSH y WiFi durante el proceso
```

### 1.2 Primera Configuración

```bash
# Conectarse por SSH
ssh pi@raspberrypi.local
# Contraseña por defecto: raspberry

# Actualizar sistema
sudo apt update
sudo apt upgrade -y

# Cambiar contraseña por seguridad
passwd

# Configurar hostname
sudo raspi-config
# System Options > Hostname > gateway-rpi-001

# Configurar timezone
sudo timedatectl set-timezone America/Bogota

# Reiniciar
sudo reboot
```

### 1.3 Instalar Dependencias del Sistema

```bash
# Conectarse nuevamente
ssh pi@gateway-rpi-001.local

# Instalar herramientas de compilación
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    sqlite3 \
    git \
    curl \
    wget \
    vim \
    htop

# Instalar Mosquitto MQTT Broker
sudo apt install -y mosquitto mosquitto-clients

# Verificar instalación de Mosquitto
mosquitto -h
mosquitto_pub --help


# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Seleccionar: 1) Proceed with installation (default)

# Cargar entorno de Rust
source $HOME/.cargo/env

# Verificar instalación
rustc --version
cargo --version
```

## Paso 2: Configurar Mosquitto MQTT

### 2.1 Configuración Básica de Mosquitto

```bash
# Crear archivo de configuración personalizado
sudo nano /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf
```

Agregar:

```conf
# Configuración MQTT para IoT Gateway
listener 1883
protocol mqtt

# Logging
# log_dest file /var/log/mosquitto/mosquitto.log
log_dest stdout
log_type error
log_type warning
log_type notice
log_type information

# Persistencia
persistence true
# persistence_location /var/lib/mosquitto/

# Límites
message_size_limit 1048576
max_keepalive 60
max_qos 2

# Permitir anónimos (cambiar en producción)
allow_anonymous true
```

### 2.2 Configurar Autenticación (Recomendado)

```bash
# Crear usuario para el gateway
sudo mosquitto_passwd -c /etc/mosquitto/passwd env_edge_gateway_rpi

# Opcional: Crear usuarios para cada sensor
sudo mosquitto_passwd /etc/mosquitto/passwd esp32_sensor_001
sudo mosquitto_passwd /etc/mosquitto/passwd esp32_sensor_002
```

Actualizar configuración:

```bash
sudo nano /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf
```

Cambiar/agregar:

```conf
# Autenticación
allow_anonymous false
password_file /etc/mosquitto/passwd
```

### 2.3 Habilitar y Probar Mosquitto

```bash
# Reiniciar Mosquitto
sudo systemctl restart mosquitto

# Habilitar inicio automático
sudo systemctl enable mosquitto

# Verificar estado
sudo systemctl status mosquitto

# Ver logs
sudo tail -f /var/log/mosquitto/mosquitto.log
```

### 2.4 Pruebas de Mosquitto

```bash
# Terminal 1: Suscribirse a todos los topics
mosquitto_sub -h localhost -t '#' -v

# Si se configuró autenticación:
mosquitto_sub -h localhost -t '#' -v -u env_edge_gateway_rpi -P password

# Terminal 2: Publicar mensaje de prueba
mosquitto_pub -h localhost -t 'test/topic' -m 'Hola MQTT'

# Con autenticación:
mosquitto_pub -h localhost -t 'test/topic' -m 'Hola MQTT' -u env_edge_gateway_rpi -P password
```

Si todo funciona correctamente, se debería ver el mensaje en Terminal 1.

## Paso 3: Compilar el Gateway

### 3.1 Clonar Repositorio

```bash
# Crear directorio para proyectos
mkdir -p ~/projects
cd ~/projects

# Clonar (o copiar) el código
git clone <repositorio> env_edge_gateway_rpi
# O si estás copiando manualmente:
# scp -r ./env_edge_gateway_rpi pi@gateway-rpi-001.local:~/projects/

cd env_edge_gateway_rpi
```

### 3.2 Configurar Variables de Entorno

```bash
# Copiar archivo de ejemplo
cp .env.example .env

# Editar configuración
nano .env
```

Configurar:

```bash
GATEWAY_ID=gateway-rpi-001
DATABASE_URL=sqlite://sensor_data.db
CLOUD_SERVICE_URL=https://cloud-service.com/api/v1/ingest
CLOUD_API_KEY=api_key_secreta_aqui
CLOUD_SYNC_BATCH_SIZE=50
CLOUD_SYNC_INTERVAL_SECS=300
DATA_RETENTION_DAYS=7

# Configuración MQTT
MQTT_BROKER_HOST=localhost
MQTT_BROKER_PORT=1883
MQTT_CLIENT_ID=gateway-rpi-mqtt-001

# Si se configuró autenticación en Mosquitto:
MQTT_USERNAME=env_edge_gateway_rpi
MQTT_PASSWORD=password_mqtt

RUST_LOG=info
```

### 3.3 Compilar el Proyecto

```bash
# Compilación optimizada para producción
# Esto puede tomar 1-10 minutos en Raspberry Pi
cargo build --release

# El binario estará en:
# ./target/release/env_edge_gateway_rpi

# Verificar tamaño del binario
ls -lh target/release/env_edge_gateway_rpi
```

### 3.4 Compilación Cruzada (Opcional - Más Rápido)

Si tienes una máquina más potente, puedes compilar allí:

```bash
# En la máquina de desarrollo (Linux/Mac)
# Instalar target para ARM
rustup target add aarch64-unknown-linux-gnu

# Instalar cross-compiler
sudo apt install gcc-aarch64-linux-gnu

# Compilar
cargo build --release --target=aarch64-unknown-linux-gnu

# Copiar binario a Raspberry Pi
scp target/aarch64-unknown-linux-gnu/release/env_edge_gateway_rpi \
    pi@gateway-rpi-001.local:~/projects/env_edge_gateway_rpi/target/release/
```

## Paso 4: Configurar como Servicio Systemd

### 4.1 Crear Archivo de Servicio

```bash
sudo nano /etc/systemd/system/env_edge_gateway_rpi.service
```

Contenido:

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

### 4.2 Habilitar e Iniciar Servicio

```bash
# Recargar configuración de systemd
sudo systemctl daemon-reload

# Habilitar inicio automático
sudo systemctl enable env_edge_gateway_rpi

# Iniciar servicio
sudo systemctl start env_edge_gateway_rpi

# Verificar estado
sudo systemctl status env_edge_gateway_rpi

# Debería mostrar:
# ● env_edge_gateway_rpi.service - IoT Gateway Edge Computing Service
#    Loaded: loaded
#    Active: active (running)
```

### 4.3 Ver Logs

```bash
# Logs en tiempo real
sudo journalctl -u env_edge_gateway_rpi -f

# Últimas 100 líneas
sudo journalctl -u env_edge_gateway_rpi -n 100

# Logs de hoy
sudo journalctl -u env_edge_gateway_rpi --since today

# Buscar errores
sudo journalctl -u env_edge_gateway_rpi -p err
```

## Paso 5: Configurar Firewall

```bash
# Instalar UFW (Uncomplicated Firewall)
sudo apt install ufw

# Permitir SSH
sudo ufw allow 22/tcp

# Permitir puerto del gateway (3000)
sudo ufw allow 3000/tcp

# Habilitar firewall
sudo ufw enable

# Verificar estado
sudo ufw status
```

## Paso 6: Probar el Gateway

### 6.1 Health Check Local (HTTP)

```bash
# Desde la Raspberry Pi
curl http://localhost:3000/health | jq

# Deberías ver:
# {
#   "status": "ok",
#   "gateway_id": "gateway-rpi-001",
#   ...
# }

curl http://localhost:2883/health | jq

# Deberías ver:
# {
#   "status": "ok",
#   "gateway_id": "gateway-rpi-001",
#   ...
# }
```

### 6.2 Probar MQTT

```bash
# Terminal 1: Suscribirse a topics de sensores
mosquitto_sub -h localhost -t 'sensors/#' -v -u env_edge_gateway_rpi -P tu_password

# Terminal 2: Publicar dato de prueba
mosquitto_pub -h localhost \
  -t 'sensors/test-sensor/data' \
  -m '{"temperature": 25.5, "humidity": 65.0}' \
  -u env_edge_gateway_rpi -P password

# Deberías ver en Terminal 1:
# - El mensaje publicado
# - La respuesta en sensors/test-sensor/processed
```

### 6.3 Probar desde la Red Local

```bash
# Desde otra máquina en la misma red
curl http://192.168.1.XXX:3000/health | jq

# Publicar via MQTT
mosquitto_pub -h 192.168.1.XXX \
  -t 'sensors/remote-test/data' \
  -m '{"temperature": 24.0, "humidity": 70.0}' \
  -u env_edge_gateway_rpi -P tu_password
```

### 6.3 Verificar Logs del Gateway

```bash
# Ver logs en tiempo real
sudo journalctl -u env_edge_gateway_rpi -f

# Buscar mensajes MQTT
sudo journalctl -u env_edge_gateway_rpi | grep MQTT

# Buscar procesamiento de datos
sudo journalctl -u env_edge_gateway_rpi | grep "Dato recibido"
```

### 6.3 Ejecutar Script de Pruebas

```bash
# Hacer ejecutable
chmod +x test_gateway.sh

# Ejecutar
./test_gateway.sh http://localhost:3000

# O desde otra máquina
./test_gateway.sh http://192.168.1.XXX:3000
```

## Paso 7: Monitoreo y Mantenimiento

### 7.1 Configurar Logrotate

```bash
sudo nano /etc/logrotate.d/env_edge_gateway_rpi
```

Contenido:

```text
/var/log/env_edge_gateway_rpi/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 pi pi
}
```

### 7.2 Script de Backup Automático

```bash
nano ~/backup_gateway.sh
```

Contenido:

```bash
#!/bin/bash
BACKUP_DIR="/home/pi/backups"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

# Backup de base de datos
sqlite3 /home/pi/projects/env_edge_gateway_rpi/sensor_data.db \
    ".backup $BACKUP_DIR/sensor_data_$DATE.db"

# Comprimir
gzip $BACKUP_DIR/sensor_data_$DATE.db

# Eliminar backups antiguos (más de 30 días)
find $BACKUP_DIR -name "sensor_data_*.db.gz" -mtime +30 -delete

echo "Backup completado: sensor_data_$DATE.db.gz"
```

```bash
# Hacer ejecutable
chmod +x ~/backup_gateway.sh

# Agregar a crontab (backup diario a las 2 AM)
crontab -e
# Agregar línea:
0 2 * * * /home/pi/backup_gateway.sh >> /home/pi/backup.log 2>&1
```

### 7.3 Monitoreo de Recursos

```bash
# Instalar herramientas
sudo apt install -y htop iotop nethogs

# Monitorear CPU/RAM
htop

# Monitorear I/O de disco
sudo iotop

# Monitorear uso de red
sudo nethogs
```

### 7.4 Alertas por Email (Opcional)

```bash
# Instalar mailutils
sudo apt install -y mailutils

# Crear script de alerta
nano ~/check_gateway.sh
```

```bash
#!/bin/bash
if ! systemctl is-active --quiet env_edge_gateway_rpi; then
    echo "IoT Gateway está caído en $(hostname)" | \
        mail -s "ALERTA: IoT Gateway Down" tu@email.com
fi
```

```bash
chmod +x ~/check_gateway.sh

# Agregar a crontab (verificar cada 5 minutos)
crontab -e
# Agregar:
*/5 * * * * /home/pi/check_gateway.sh
```

## Paso 8: Actualizaciones

### 8.1 Actualizar el Gateway

```bash
# Detener servicio
sudo systemctl stop env_edge_gateway_rpi

# Actualizar código
cd ~/projects/env_edge_gateway_rpi
git pull origin main

# Recompilar
cargo build --release

# Reiniciar servicio
sudo systemctl start env_edge_gateway_rpi

# Verificar
sudo systemctl status env_edge_gateway_rpi
curl http://localhost:3000/health
```

### 8.2 Actualización Automática del Sistema

```bash
# Instalar unattended-upgrades
sudo apt install unattended-upgrades

# Configurar
sudo dpkg-reconfigure unattended-upgrades
# Seleccionar: Yes
```

## Troubleshooting

### Problema: Gateway no inicia

```bash
# Ver logs detallados
sudo journalctl -u env_edge_gateway_rpi -n 100

# Verificar permisos
ls -l ~/projects/env_edge_gateway_rpi/target/release/env_edge_gateway_rpi

# Ejecutar manualmente para ver errores
cd ~/projects/env_edge_gateway_rpi
./target/release/env_edge_gateway_rpi
```

### Problema: Base de datos corrupta

```bash
# Verificar integridad
sqlite3 sensor_data.db "PRAGMA integrity_check;"

# Si está corrupta, restaurar backup
cp ~/backups/sensor_data_YYYYMMDD.db.gz .
gunzip sensor_data_YYYYMMDD.db.gz
mv sensor_data_YYYYMMDD.db sensor_data.db

# Reiniciar servicio
sudo systemctl restart env_edge_gateway_rpi
```

### Problema: Sin sincronización con cloud

```bash
# Verificar conectividad
curl -I https://cloud-service.com/api/v1/health

# Verificar API key
echo $CLOUD_API_KEY

# Ver logs de sincronización
sudo journalctl -u env_edge_gateway_rpi | grep "sincronización"

# Probar manualmente
curl -X POST https://cloud-service.com/api/v1/ingest \
    -H "Authorization: Bearer $CLOUD_API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"test": "data"}'
```

### Problema: Alto uso de CPU

```bash
# Verificar procesos
htop

# Reducir nivel de logging
# Editar .env:
RUST_LOG=warn

# Reiniciar
sudo systemctl restart env_edge_gateway_rpi
```

### Problema: Disco lleno

```bash
# Ver uso de disco
df -h

# Limpiar datos antiguos sincronizados
sqlite3 sensor_data.db "DELETE FROM sensor_readings WHERE synced=1 AND datetime(gateway_timestamp) < datetime('now', '-7 days');"

# Vacuum de la base de datos
sqlite3 sensor_data.db "VACUUM;"

# Limpiar logs antiguos
sudo journalctl --vacuum-time=7d
```

## Seguridad en Producción

### 1. Cambiar Puerto por Defecto

```bash
# Editar servicio
sudo nano /etc/systemd/system/env_edge_gateway_rpi.service

# Agregar variable de entorno
Environment="PORT=8080"

# En el código, leer PORT del entorno
```

### 2. Configurar HTTPS con Nginx

```bash
# Instalar Nginx
sudo apt install nginx

# Configurar reverse proxy
sudo nano /etc/nginx/sites-available/env_edge_gateway_rpi
```

```nginx
server {
    listen 80;
    server_name env_edge_gateway_rpi.local;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### 3. Rate Limiting

Agregar al proyecto:

```toml
# En Cargo.toml
tower-governor = "0.1"
```

### 4. Fail2ban

```bash
# Instalar fail2ban
sudo apt install fail2ban

# Configurar para proteger SSH
sudo cp /etc/fail2ban/jail.conf /etc/fail2ban/jail.local
sudo systemctl enable fail2ban
sudo systemctl start fail2ban
```

## Optimizaciones de Performance

### 1. Usar tmpfs para Base de Datos Temporal

```bash
# Editar fstab
sudo nano /etc/fstab

# Agregar:
tmpfs /tmp tmpfs defaults,noatime,nosuid,size=256m 0 0

# Reiniciar
sudo reboot
```

### 2. Deshabilitar Servicios Innecesarios

```bash
# Listar servicios
systemctl list-unit-files --type=service --state=enabled

# Deshabilitar servicios no usados
sudo systemctl disable bluetooth
sudo systemctl disable avahi-daemon
```

## Checklist de Deployment

- [ ] Raspberry Pi actualizada y configurada
- [ ] Rust instalado y funcional
- [ ] Código compilado exitosamente
- [ ] Variables de entorno configuradas
- [ ] Servicio systemd creado y habilitado
- [ ] Firewall configurado
- [ ] Health check funcionando
- [ ] Tests pasando
- [ ] Logs configurados
- [ ] Backup automático configurado
- [ ] Monitoreo en lugar
- [ ] Documentación revisada
- [ ] ESP32 sensores configurados y conectados

## Soporte

Si encuentras problemas:

1. Revisar logs: `sudo journalctl -u env_edge_gateway_rpi -f`
2. Verificar health: `curl http://localhost:3000/health`
3. Revisar issues en el repositorio
4. Contactar al equipo de desarrollo

---

**¡Gateway IoT desplegado exitosamente!** 🎉
