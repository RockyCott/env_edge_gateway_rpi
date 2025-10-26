# Dockerfile para compilar y ejecutar en Raspberry Pi

# Stage 1: Builder
FROM rust:1.85-slim as builder

# Instalar dependencias de compilación
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copiar manifiestos
COPY Cargo.toml Cargo.lock ./

# Copiar código fuente
COPY src ./src

# Compilar en modo release
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Instalar dependencias de runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copiar binario compilado
COPY --from=builder /app/target/release/env_edge_gateway_rpi /app/env_edge_gateway_rpi

# Crear directorio para base de datos
RUN mkdir -p /app/data

# Variables de entorno por defecto
ENV DATABASE_URL=sqlite:///app/data/sensor_data.db
ENV RUST_LOG=info

# Exponer puerto
EXPOSE 3000

# Ejecutar
CMD ["/app/env_edge_gateway_rpi"]