#!/bin/bash

# Script de prueba para el IoT Gateway
# Simula sensores ESP32 enviando datos

GATEWAY_URL="${1:-http://localhost:3000}"

echo "Probando IoT Gateway en: $GATEWAY_URL"
echo ""

# Colores para output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test 1: Health Check
echo -e "${YELLOW}Test 1: Health Check${NC}"
response=$(curl -s -w "\n%{http_code}" "$GATEWAY_URL/health")
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" -eq 200 ]; then
    echo -e "${GREEN}✓ Health check exitoso${NC}"
    echo "$body" | jq '.'
else
    echo -e "${RED}✗ Health check falló (HTTP $http_code)${NC}"
    exit 1
fi
echo ""

# Test 2: Enviar datos individuales
echo -e "${YELLOW}Test 2: Enviar datos individuales${NC}"
payload='{
  "sensor_id": "test-sensor-001",
  "temperature": 25.5,
  "humidity": 65.0,
  "battery_level": 85.0,
  "rssi": -65
}'

response=$(curl -s -w "\n%{http_code}" \
  -X POST "$GATEWAY_URL/api/v1/sensor/data" \
  -H "Content-Type: application/json" \
  -d "$payload")

http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" -eq 200 ]; then
    echo -e "${GREEN}✓ Datos enviados exitosamente${NC}"
    echo "$body" | jq '.'
else
    echo -e "${RED}✗ Envío falló (HTTP $http_code)${NC}"
    echo "$body"
fi
echo ""

# Test 3: Enviar batch de datos
echo -e "${YELLOW}Test 3: Enviar batch de datos${NC}"
batch_payload='{
  "readings": [
    {
      "sensor_id": "test-sensor-001",
      "temperature": 26.0,
      "humidity": 67.0,
      "battery_level": 84.0,
      "rssi": -66
    },
    {
      "sensor_id": "test-sensor-002",
      "temperature": 24.5,
      "humidity": 62.0,
      "battery_level": 90.0,
      "rssi": -60
    },
    {
      "sensor_id": "test-sensor-003",
      "temperature": 27.0,
      "humidity": 70.0,
      "battery_level": 75.0,
      "rssi": -70
    }
  ]
}'

response=$(curl -s -w "\n%{http_code}" \
  -X POST "$GATEWAY_URL/api/v1/sensor/batch" \
  -H "Content-Type: application/json" \
  -d "$batch_payload")

http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" -eq 200 ]; then
    echo -e "${GREEN}✓ Batch enviado exitosamente${NC}"
    echo "$body" | jq '.'
else
    echo -e "${RED}✗ Batch falló (HTTP $http_code)${NC}"
    echo "$body"
fi
echo ""

# Test 4: Obtener métricas
echo -e "${YELLOW}Test 4: Obtener métricas${NC}"
response=$(curl -s -w "\n%{http_code}" "$GATEWAY_URL/metrics")
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" -eq 200 ]; then
    echo -e "${GREEN}✓ Métricas obtenidas${NC}"
    echo "$body" | jq '.'
else
    echo -e "${RED}✗ Obtención de métricas falló (HTTP $http_code)${NC}"
fi
echo ""

# Test 5: Datos recientes
echo -e "${YELLOW}Test 5: Consultar datos recientes${NC}"
response=$(curl -s -w "\n%{http_code}" \
  "$GATEWAY_URL/api/v1/data/recent?sensor_id=test-sensor-001&limit=5")
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" -eq 200 ]; then
    echo -e "${GREEN}✓ Datos recientes obtenidos${NC}"
    echo "$body" | jq '.'
else
    echo -e "${RED}✗ Consulta falló (HTTP $http_code)${NC}"
fi
echo ""

# Test 6: Validación - Temperatura fuera de rango
echo -e "${YELLOW}Test 6: Validación (temperatura inválida)${NC}"
invalid_payload='{
  "sensor_id": "test-sensor-001",
  "temperature": 150.0,
  "humidity": 65.0
}'

response=$(curl -s -w "\n%{http_code}" \
  -X POST "$GATEWAY_URL/api/v1/sensor/data" \
  -H "Content-Type: application/json" \
  -d "$invalid_payload")

http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" -eq 400 ]; then
    echo -e "${GREEN}✓ Validación funciona correctamente (rechazó dato inválido)${NC}"
    echo "$body" | jq '.'
else
    echo -e "${RED}✗ Validación no funcionó como esperado${NC}"
fi
echo ""

# Test 7: Stress test - Múltiples requests
echo -e "${YELLOW}Test 7: Stress test (50 requests)${NC}"
success=0
failures=0

for i in {1..50}; do
    temp=$(awk -v min=20 -v max=30 'BEGIN{srand(); print min+rand()*(max-min)}')
    hum=$(awk -v min=40 -v max=80 'BEGIN{srand(); print min+rand()*(max-min)}')
    
    payload="{\"sensor_id\": \"stress-test-$((i % 5))\", \"temperature\": $temp, \"humidity\": $hum}"
    
    http_code=$(curl -s -w "%{http_code}" -o /dev/null \
      -X POST "$GATEWAY_URL/api/v1/sensor/data" \
      -H "Content-Type: application/json" \
      -d "$payload")
    
    if [ "$http_code" -eq 200 ]; then
        ((success++))
    else
        ((failures++))
    fi
    
    # Progress bar
    if [ $((i % 10)) -eq 0 ]; then
        echo -n "."
    fi
done

echo ""
echo -e "${GREEN}Exitosos: $success${NC}"
echo -e "${RED}Fallidos: $failures${NC}"
echo ""

# Resumen final
echo "================================================"
echo -e "${GREEN}✓ Tests completados${NC}"
echo "================================================"
echo ""
echo "Para monitoreo continuo, ejecuta:"
echo "  watch -n 5 'curl -s $GATEWAY_URL/health | jq'"
echo ""
echo "Para ver logs en tiempo real:"
echo "  journalctl -u env_edge_gateway_rpi -f"