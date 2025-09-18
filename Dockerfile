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
RUN --mount=type=secret,id=ANTHROPIC_API_KEY ANTHROPIC_API_KEY="$(cat /run/secrets/ANTHROPIC_API_KEY)" cargo build --release -p site

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/site /usr/local/bin
# Copy assets folder for static files (CSS, icons, media)
COPY --from=builder /app/site/assets ./assets
ENTRYPOINT ["/usr/local/bin/site"]
