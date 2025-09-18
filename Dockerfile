FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --manifest-path "site/Cargo.toml" --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release -p site

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/site /usr/local/bin
# Copy assets folder for static files (CSS, icons, media)
COPY --from=builder /app/site/assets ./assets
# Copy .env file for environment configuration
COPY --from=builder /app/.env ./.env
ENTRYPOINT ["/usr/local/bin/site"]
