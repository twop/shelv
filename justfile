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

# generate src/scripting/system.d.ts (typings for editors and the AI prompt) from system.ts (needs tsc);
# Shelv embeds the .ts files themselves and strips their types at load
build-system:
    tsc src/scripting/system.ts --target es2022 --module esnext --noCheck --declaration --emitDeclarationOnly --stripInternal --outDir src/scripting

# build-system, then type-check the files in src/scripting/tsconfig.json (needs tsc)
typecheck-settings: build-system
    tsc -p src/scripting

# Shelv.app (release, or `just dev-bundle debug`) with document types registered
dev-bundle profile="release":
    distribution/dev_bundle.sh {{profile}}
