#!/bin/bash

# Script de prueba para el sistema MQTT del IoT Gateway
# Simula sensores ESP32 publicando datos via MQTT

MQTT_BROKER="${1:-localhost}"
MQTT_PORT="${2:-1883}"
MQTT_USER="${3:-}"
MQTT_PASS="${4:-}"

echo "Probando Sistema MQTT del IoT Gateway"
echo "========================================"
echo "Broker: $MQTT_BROKER:$MQTT_PORT"
echo ""

# Colores
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Verificar si mosquitto_pub y mosquitto_sub están instalados
if ! command -v mosquitto_pub &> /dev/null; then
    echo -e "${RED}mosquitto_pub no está instalado${NC}"
    echo "Instalar con: sudo apt install mosquitto-clients"
    exit 1
fi

# Construir argumentos de autenticación
AUTH_ARGS=""
if [ -n "$MQTT_USER" ]; then
    AUTH_ARGS="-u $MQTT_USER -P $MQTT_PASS"
fi

# Test 1: Conectividad básica
echo -e "${YELLOW}Test 1: Verificar conectividad con broker MQTT${NC}"
if timeout 5 mosquitto_pub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS -t "test/connectivity" -m "ping" 2>/dev/null; then
    echo -e "${GREEN}Conectividad OK${NC}"
else
    echo -e "${RED}No se puede conectar al broker${NC}"
    echo "Verificar:"
    echo "  - Mosquitto está corriendo: sudo systemctl status mosquitto"
    echo "  - Firewall permite puerto 1883: sudo ufw status"
    echo "  - Credenciales son correctas"
    exit 1
fi
echo ""

# Test 2: Publicar dato individual
echo -e "${YELLOW}Test 2: Publicar dato individual${NC}"
SENSOR_ID="test-sensor-$(date +%s)"
TOPIC_DATA="sensors/$SENSOR_ID/data"
TOPIC_PROCESSED="sensors/$SENSOR_ID/processed"

PAYLOAD='{"temperature": 25.5, "humidity": 65.0, "battery_level": 85.0, "rssi": -65}'

echo "Topic: $TOPIC_DATA"
echo "Payload: $PAYLOAD"

# Suscribirse a respuesta en background
(mosquitto_sub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS \
    -t "$TOPIC_PROCESSED" -C 1 -W 5 2>/dev/null | head -n 1) &
SUB_PID=$!

# Dar tiempo para suscribirse
sleep 1

# Publicar
if mosquitto_pub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS \
    -t "$TOPIC_DATA" -m "$PAYLOAD"; then
    echo -e "${GREEN}Dato publicado${NC}"
    
    # Esperar respuesta
    sleep 2
    if ps -p $SUB_PID > /dev/null 2>&1; then
        echo -e "${YELLOW}Esperando respuesta del gateway...${NC}"
        wait $SUB_PID
        RESPONSE=$?
        if [ $RESPONSE -eq 0 ]; then
            echo -e "${GREEN}Respuesta recibida del gateway${NC}"
        else
            echo -e "${RED}No se recibió respuesta (timeout 5s)${NC}"
            echo "El gateway puede no estar procesando mensajes MQTT"
        fi
    fi
else
    echo -e "${RED}Error publicando dato${NC}"
fi
echo ""

# Test 3: Publicar batch
echo -e "${YELLOW}Test 3: Publicar batch de datos${NC}"
TOPIC_BATCH="sensors/$SENSOR_ID/batch"
TOPIC_BATCH_PROCESSED="sensors/$SENSOR_ID/batch_processed"

BATCH_PAYLOAD='{
  "readings": [
    {"temperature": 25.0, "humidity": 60.0, "battery_level": 85.0, "rssi": -65},
    {"temperature": 25.5, "humidity": 62.0, "battery_level": 84.0, "rssi": -66},
    {"temperature": 26.0, "humidity": 64.0, "battery_level": 83.0, "rssi": -67}
  ]
}'

echo "Topic: $TOPIC_BATCH"
echo "Lecturas en batch: 3"

# Suscribirse a respuesta de batch
(mosquitto_sub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS \
    -t "$TOPIC_BATCH_PROCESSED" -C 1 -W 5 2>/dev/null | head -n 1) &
SUB_PID=$!

sleep 1

if mosquitto_pub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS \
    -t "$TOPIC_BATCH" -m "$BATCH_PAYLOAD"; then
    echo -e "${GREEN}Batch publicado${NC}"
    
    sleep 2
    if ps -p $SUB_PID > /dev/null 2>&1; then
        echo -e "${YELLOW}Esperando respuesta del gateway...${NC}"
        wait $SUB_PID
        RESPONSE=$?
        if [ $RESPONSE -eq 0 ]; then
            echo -e "${GREEN}Respuesta de batch recibida${NC}"
        else
            echo -e "${RED}No se recibió respuesta de batch${NC}"
        fi
    fi
else
    echo -e "${RED}Error publicando batch${NC}"
fi
echo ""

# Test 4: Stress test
echo -e "${YELLOW}Test 4: Stress test (50 mensajes)${NC}"
SUCCESS=0
FAILURES=0

for i in {1..50}; do
    TEMP=$(awk -v min=20 -v max=30 'BEGIN{srand(); printf "%.1f", min+rand()*(max-min)}')
    HUM=$(awk -v min=40 -v max=80 'BEGIN{srand(); printf "%.1f", min+rand()*(max-min)}')
    
    PAYLOAD="{\"temperature\": $TEMP, \"humidity\": $HUM, \"battery_level\": 85.0, \"rssi\": -65}"
    
    if mosquitto_pub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS \
        -t "sensors/stress-test-$((i % 5))/data" -m "$PAYLOAD" 2>/dev/null; then
        ((SUCCESS++))
    else
        ((FAILURES++))
    fi
    
    # Progress
    if [ $((i % 10)) -eq 0 ]; then
        echo -n "."
    fi
    
    # Pequeño delay para no saturar
    sleep 0.1
done

echo ""
echo -e "${GREEN}Exitosos: $SUCCESS${NC}"
if [ $FAILURES -gt 0 ]; then
    echo -e "${RED}Fallidos: $FAILURES${NC}"
else
    echo -e "${GREEN}Fallidos: $FAILURES${NC}"
fi
echo ""

# Test 5: Monitorear topics en tiempo real
echo -e "${YELLOW}Test 5: Monitoreo de topics (5 segundos)${NC}"
echo "Suscribiéndose a sensors/# ..."
echo "Publicando algunos mensajes de prueba..."
echo ""

# Iniciar monitoreo en background
(mosquitto_sub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS \
    -t "sensors/#" -v 2>/dev/null) &
MONITOR_PID=$!

sleep 1

# Publicar algunos mensajes
for i in {1..3}; do
    mosquitto_pub -h $MQTT_BROKER -p $MQTT_PORT $AUTH_ARGS \
        -t "sensors/monitor-test/data" \
        -m "{\"temperature\": 2$i.5, \"humidity\": 6$i.0}" 2>/dev/null
    sleep 1
done

sleep 2

# Detener monitoreo
kill $MONITOR_PID 2>/dev/null
wait $MONITOR_PID 2>/dev/null

echo ""
echo -e "${GREEN}Test de monitoreo completado${NC}"
echo ""

# Resumen
echo "================================================"
echo -e "${GREEN}Tests MQTT completados${NC}"
echo "================================================"
echo ""
echo "Verificaciones adicionales:"
echo ""
echo "1. Ver logs del gateway:"
echo "   sudo journalctl -u env_edge_gateway_rpi -f"
echo ""
echo "2. Verificar datos en base de datos:"
echo "   sqlite3 ~/projects/env_edge_gateway_rpi/sensor_data.db"
echo "   SELECT COUNT(*) FROM sensor_readings;"
echo ""
echo "3. Monitorear todos los topics MQTT:"
echo "   mosquitto_sub -h $MQTT_BROKER -t '#' -v $AUTH_ARGS"
echo ""
echo "4. Ver estadísticas de Mosquitto:"
echo "   mosquitto_sub -h $MQTT_BROKER -t '\$SYS/#' -v $AUTH_ARGS"
echo ""
echo "5. Health check HTTP:"
echo "   curl http://$MQTT_BROKER:2883/health | jq"
echo ""
echo "================================================"