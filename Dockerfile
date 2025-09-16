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

# Install kubectl
RUN curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl" \
    && chmod +x kubectl \
    && mv kubectl /usr/local/bin/

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

# Set only essential environment variables (others configured via Kubernetes ConfigMap)
ENV RUST_LOG=info
ENV BIND_ADDR=0.0.0.0:8080
ENV CASHU_DB_PATH=/app/data/cashu.db

# Health check (disabled for Nostr mode - no HTTP endpoints)
# HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
#     CMD curl -f http://localhost:8080/healthz || exit 1

# Run the service
CMD ["paygress-sidecar"]
