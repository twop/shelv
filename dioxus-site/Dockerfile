# Build stage
FROM rust:bookworm AS builder

ARG APPNAME=dioxus-site
ARG OUTDIR=dist

# Install build tools
RUN rustup target add wasm32-unknown-unknown
RUN cargo install --version 0.5.7 dioxus-cli --locked

WORKDIR /app

# Copy the entire project
COPY . .

# # Build Tailwind CSS
# FROM node:22-alpine as tailwind
# WORKDIR /app
# COPY . .
# RUN npx tailwindcss -i ./input.css -o ./assets/tailwind.css --minify

# # Back to the builder stage
FROM builder AS build
# COPY --from=tailwind /app/assets/tailwind.css ./public/tailwind.css

# Build the project
RUN dx build --platform fullstack --release

# Runtime stage
FROM debian:bookworm-slim AS runtime

ARG OUTDIR
ARG APPNAME

WORKDIR /usr/local/bin
RUN apt-get update \
    && apt-get install -y libssl-dev pkg-config ca-certificates \
    && apt-get clean && update-ca-certificates

COPY --from=build /app/$OUTDIR /usr/local/bin
# COPY --from=build /app/config /usr/local/bin/config

EXPOSE 8080

ENTRYPOINT ["./target/release/dioxus-site"]


# ARG RUST_VERSION=1.79.0

# # Create a stage for building the application.

# FROM rust:${RUST_VERSION}-alpine AS build
# WORKDIR /

# # Install host build dependencies.
# RUN apk add --no-cache clang lld musl-dev git pkgconfig perl make libressl-dev

# # Build the application.
# # Leverage a cache mount to /usr/local/cargo/registry/
# # for downloaded dependencies, a cache mount to /usr/local/cargo/git/db
# # for git repository dependencies, and a cache mount to /app/target/ for
# # compiled dependencies which will speed up subsequent builds.
# # Leverage a bind mount to the src directory to avoid having to copy the
# # source code into the container. Once built, copy the executable to an
# # output directory before the cache mounted /app/target is unmounted.
# RUN --mount=type=bind,source=src,target=src \
#     --mount=type=bind,source=public,target=public \
#     --mount=type=bind,source=assets,target=assets \
#     --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
#     --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
#     --mount=type=bind,source=Dioxus.toml,target=Dioxus.toml \
#     --mount=type=bind,source=input.css,target=input.css \
#     --mount=type=bind,source=tailwind.config.js,target=tailwind.config.js \
#     --mount=type=cache,target=/app/target/ \
#     --mount=type=cache,target=/usr/local/cargo/git/db \
#     --mount=type=cache,target=/usr/local/cargo/registry/ \
#     cargo install dioxus-cli && \
#     dx build --platform fullstack --release

# ######################################################
# # Create a new stage for running the application that contains the minimal
# # runtime dependencies for the application. This often uses a different base
# # image from the build stage where the necessary files are copied from the build
# # stage.

# FROM alpine:3.18 AS final

# # Copy the executable from the "build" stage.
# COPY --from=build . .

# # Expose the port that the application listens on.
# EXPOSE 8080

# # What the container should run when it is started.
# CMD ["./target/release/smp-fs"]
