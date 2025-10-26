#!/bin/bash

# Script para instalar y configurar Mosquitto en Raspberry Pi
# Para el IoT Gateway Edge Computing

echo "================================================"
echo "  Instalación de Mosquitto MQTT Broker"
echo "  Para IoT Gateway en Raspberry Pi"
echo "================================================"
echo ""

# Verificar que se ejecuta como root
if [ "$EUID" -ne 0 ]; then 
    echo " Este script debe ejecutarse como root"
    echo " Ejecuta: sudo bash install_mosquitto.sh"
    exit 1
fi

echo "Actualizando repositorios..."
apt update

echo ""
echo "Instalando Mosquitto y cliente..."
apt install -y mosquitto mosquitto-clients

echo ""
echo "Mosquitto instalado"
echo ""

# Crear archivo de configuración
echo "Configurando Mosquitto..."

# Backup de configuración original
if [ -f /etc/mosquitto/mosquitto.conf ]; then
    cp /etc/mosquitto/mosquitto.conf /etc/mosquitto/mosquitto.conf.backup
    echo "   Backup de configuración creado"
fi

# Crear configuración personalizada
cat > /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf << 'EOF'
# Configuración MQTT para IoT Gateway
# Archivo: /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf

# Listener en puerto estándar
listener 1883
protocol mqtt

# Permitir conexiones anónimas (cambiar en producción)
allow_anonymous true

# Logging
# log_dest file /var/log/mosquitto/mosquitto.log
log_dest stdout
log_type error
log_type warning
log_type notice
log_type information

# Persistencia de mensajes
persistence true
# persistence_location /var/lib/mosquitto/

# Archivo de estado
pid_file /run/mosquitto/mosquitto.pid

# Tamaño máximo de mensaje (1MB)
message_size_limit 1048576

# Keepalive
max_keepalive 60

# Límite de conexiones
max_connections -1

# QoS máximo
max_qos 2
EOF

echo "   Configuración personalizada creada"

# Crear archivo de contraseñas (opcional)
echo ""
echo "Configurando autenticación..."
read -p "¿Configurar autenticación con usuario/contraseña? (s/n): " setup_auth

if [ "$setup_auth" = "s" ] || [ "$setup_auth" = "S" ]; then
    read -p "Usuario MQTT: " mqtt_user
    
    # Crear archivo de contraseñas
    mosquitto_passwd -c /etc/mosquitto/passwd "$mqtt_user"
    
    # Actualizar configuración para requerir autenticación
    cat >> /etc/mosquitto/conf.d/env_edge_gateway_rpi.conf << EOF

# Autenticación
allow_anonymous false
password_file /etc/mosquitto/passwd
EOF
    
    echo "   Autenticación configurada para usuario: $mqtt_user"
    echo "    Actualiza .env con:"
    echo "    MQTT_USERNAME=$mqtt_user"
    echo "    MQTT_PASSWORD=tu_password"
else
    echo "    Autenticación no configurada (allow_anonymous=true)"
    echo "    Para producción, se recomienda habilitar autenticación"
fi

echo ""
echo "Reiniciando Mosquitto..."
systemctl restart mosquitto

echo ""
echo "Habilitando inicio automático..."
systemctl enable mosquitto

echo ""
echo "Estado del servicio:"
systemctl status mosquitto --no-pager

echo ""
echo "================================================"
echo "  Mosquitto instalado y configurado"
echo "================================================"
echo ""
echo "Pruebas:"
echo ""
echo "1. Verificar que está escuchando:"
echo "   netstat -tulpn | grep 1883"
echo ""
echo "2. Suscribirse a un topic (terminal 1):"
echo "   mosquitto_sub -h localhost -t 'test/#' -v"
echo ""
echo "3. Publicar un mensaje (terminal 2):"
echo "   mosquitto_pub -h localhost -t 'test/topic' -m 'Hello World'"
echo ""
echo "4. Ver logs:"
echo "   tail -f /var/log/mosquitto/mosquitto.log"
echo ""
echo "Configuración del Gateway:"
echo "  En el archivo .env, configurar:"
echo "  MQTT_BROKER_HOST=localhost"
echo "  MQTT_BROKER_PORT=1883"
echo ""
echo "Firewall (si usas UFW):"
echo "  sudo ufw allow 1883/tcp"
echo ""
echo "================================================"