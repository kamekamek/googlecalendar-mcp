# Build stage
FROM rustlang/rust:nightly-slim AS builder

WORKDIR /app

# Avoid rustup self-updates during Docker builds (can fail on overlay filesystems)
ENV RUSTUP_DISABLE_SELF_UPDATE=1 \
    RUSTUP_SKIP_SELF_UPDATE=1 \
    RUSTUP_NO_UPDATE_CHECK=1

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true
RUN rm -rf src

# Build application
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/mcp_google_calendar /app/mcp_google_calendar
COPY config/config.toml /app/config/config.toml

# Cloud Run default port
ENV PORT=8080
ENV APP__SERVER__BIND_ADDRESS="0.0.0.0:8080"
ENV APP__PROXY__ENABLED="true"

EXPOSE 8080

CMD ["/app/mcp_google_calendar"]
