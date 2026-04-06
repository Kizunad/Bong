# Bong

AI-Native Xianxia (修仙) sandbox on Minecraft. Three-layer architecture:

- **server/** — Rust headless MC server (Valence on Bevy 0.14 ECS, MC 1.20.1 protocol 763)
- **client/** — Fabric 1.20.1 micro-client (Java 17, owo-lib for UI)
- **agent/** — LLM "天道" agent layer (TypeScript, planned Pi framework fork)

## Quick commands

```bash
# Server
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd server && cargo run              # listens on :25565, offline mode

# Client
cd client && ./gradlew test build   # jar in build/libs/
cd client && ./gradlew runClient    # launches MC via WSLg

# Schema (TS side)
cd agent/packages/schema && npm test

# Full smoke test
bash scripts/smoke-test.sh
```

## Key dependencies & versions

- Valence: git rev `2b705351` (pinned in Cargo.toml)
- big-brain `0.21`, bevy_transform `0.14.2`, pathfinding `4`
- Fabric: MC 1.20.1, Loader 0.16.10, owo-lib 0.11.2+1.20
- Schema: @sinclair/typebox 0.34

## Architecture notes

- Server ↔ Agent communication: crossbeam channels (in-process mock for MVP), planned Redis IPC
- IPC schema defined as TypeBox (TS source of truth) → JSON Schema export → Rust serde structs
- Shared validation via `agent/packages/schema/samples/*.json` — both TS tests and Rust `include_str!` tests use these
- NPC AI: big-brain Utility AI (Scorer → Action pattern), Position↔Transform sync bridge
- `#[allow(dead_code)]` on `mod schema` in main.rs — schema module is for IPC alignment, not yet wired into runtime

## Conventions

- Communicate in Chinese (中文)
- User develops on cloud, pulls to local WSL for testing
- `cargo run` uses offline mode (no Mojang auth needed)
- Client testing via `./gradlew runClient` (WSLg, no separate launcher)
- Java 17 for Fabric, Java 21 as system default (sdkman)
