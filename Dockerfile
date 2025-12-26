# Paygress Dockerfile
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency files
COPY Cargo.toml Cargo.lock ./

# Create dummy src to build dependencies first (caching)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source code
COPY src/ src/

# Build the application in release mode
# Use 'touch' to ensure cargo rebuilds the main binary
RUN touch src/main.rs && cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies and utilities
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    iptables \
    iproute2 \
    && rm -rf /var/lib/apt/lists/*

# Install kubectl
RUN curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl" \
    && chmod +x kubectl \
    && mv kubectl /usr/local/bin/

# Create non-root user (optional, but for host networking + k8s access, running as root might be needed 
# or specific permissions. Sticking to root for now given the low-level networking requirements 
# or ensuring the user has permissions. For simplicity in baremetal provisioning: keep root or ensure sudo).
# Ideally we run as a user, but 'paygress' needs to spawn k8s pods which might cache kubeconfig owned by root/others.
# Let's clean up:
RUN mkdir -p /app/data

# Copy the binary from builder stage (FIXED binary name)
COPY --from=builder /app/target/release/paygress /usr/local/bin/paygress

# Set working directory
WORKDIR /app

# Expose the service port (documentation only when using host network)
EXPOSE 8080

# Environment variables (Defaults, override with .env)
ENV RUST_LOG=info
# BIND_ADDR and CASHU_DB_PATH should be set in .env or via orchestration
# Keeping sensible defaults for standalone runs if needed, or removing to force config.
# Let's keep generic defaults but remove specific paths that might not exist without volume mounts if not careful.
ENV BIND_ADDR=0.0.0.0:8080
# CASHU_DB_PATH is better set at runtime to ensure it matches volume mounts


# Run the service
CMD ["paygress"]
