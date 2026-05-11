# valence_generated

Contains the majority of Valence's generated Rust code for use with Minecraft's protocol and entity data.
This is currently meant to be an implementation detail of `valence_protocol` and `valence_entity`.

This directory is Bong's fork of upstream `valence_generated` at Valence rev `2b705351`.
The crate name stays `valence_generated` because `server/Cargo.toml` loads it through a Cargo `[patch]`.

## Bong custom block checklist

This fork keeps vanilla `extracted/blocks.json` unchanged. Bong-only blocks are appended from the repository-root
`bong_blocks.json`; IDs are assigned after the vanilla registry in file order.

To add one Bong custom block:

1. Append the block definition to `bong_blocks.json`.
2. Run `cargo test` in `server/crates/valence_generated_bong` and verify the generated `BlockState::BONG_*`
   constant and `BlockKind::Bong*` variant.
3. Add or update the client registration in `client/src/main/java/com/bong/client/block/BongBlocks.java`.
4. Add matching `client/src/main/resources/assets/bong/blockstates/*.json`,
   `client/src/main/resources/assets/bong/models/block/*.json`, and
   `client/src/main/resources/assets/bong/textures/block/*.png`.
5. Run `./gradlew generateBongBlockIds test --tests com.bong.client.block.BongBlocksTest` from `client/`
   with Java 17; the generated `BongBlockIds` class must stay aligned with the server state IDs.
6. Place the block through a server path that uses `world::bong_blocks::place_bong_block`, then verify the
   Fabric client starts without raw ID mismatch.
