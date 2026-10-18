# rust-lld can't link against the macOS 27 SDK, so pin an older one.
# Override with: just sdkroot=/path/to/sdk <recipe>  (or set SHELV_SDKROOT)
sdkroot := env_var_or_default("SHELV_SDKROOT", "/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk")

export SDKROOT := sdkroot

default:
    @just --list

check *args:
    cargo check {{args}}

build *args:
    cargo build {{args}}

release *args:
    cargo build --release {{args}}

run *args:
    cargo run {{args}}

test *args:
    cargo test {{args}}

clippy *args:
    cargo clippy {{args}}

# debug Shelv.app with document types registered
dev-bundle:
    distribution/dev_bundle.sh
