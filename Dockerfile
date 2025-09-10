# Paygress Sidecar Service Dockerfile
FROM rust:1.85 AS builder

WORKDIR /app

# Copy dependency files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ src/

# Build the application in release mode
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install required packages
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false -u 1000 paygress

# Create data directory
RUN mkdir -p /app/data && chown paygress:paygress /app/data

# Copy the binary from builder stage
COPY --from=builder /app/target/release/paygress-sidecar /usr/local/bin/paygress-sidecar

# Set ownership and permissions
RUN chmod +x /usr/local/bin/paygress-sidecar

# Switch to non-root user
USER paygress

# Set working directory
WORKDIR /app

# Expose the service port
EXPOSE 8080

# Set default environment variables
ENV PAYGRESS_MODE=nostr
ENV BIND_ADDR=0.0.0.0:8080
ENV CASHU_DB_PATH=/app/data/cashu.db
ENV POD_NAMESPACE=user-workloads
ENV PAYMENT_RATE_SATS_PER_HOUR=100
ENV DEFAULT_POD_DURATION_MINUTES=60
ENV SSH_BASE_IMAGE=linuxserver/openssh-server:latest
ENV SSH_PORT=2222
ENV ENABLE_CLEANUP_TASK=true
ENV RUST_LOG=info

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/healthz || exit 1

# Run the service
CMD ["paygress-sidecar"]
