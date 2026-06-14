//! `bong-server` as a library crate.
//!
//! The server ships as a binary (`src/main.rs`), but the module tree lives here
//! in a `lib` target so out-of-crate integration targets — notably the criterion
//! performance benches under `benches/` (worldgen-v4 §8.1 #10) — can reach the
//! real production code paths (`world::terrain::column::fill_column`,
//! `world::terrain::nbt_registry::DecorationNbtRegistry::stamp`) without resorting
//! to duplicated logic or subprocess timing. The thin binary in `main.rs` consumes
//! these modules exactly as before; `crate::` paths inside every module resolve
//! against this lib root unchanged.
//!
//! Every module is `pub` so the binary (a separate crate) can call each
//! `register(&mut App)`. This is the standard lib+bin split; it changes no
//! runtime behaviour.
//!
//! `private_interfaces` is allowed at the crate root: many Bevy ECS systems are
//! `pub fn`s whose signatures mention crate-internal resource / query types
//! (`pub(crate) struct AudioEmitWriter`, `SpiritNicheRegistry`, …). Under the old
//! private `mod` tree those `pub fn`s were only ever crate-visible, so the lint
//! never fired. Promoting the module tree to `pub` (so the bin + benches can
//! reach it) surfaces them. These types are never meant for cross-crate use — the
//! lib is a bin/bench-support shim, not a published API — so silencing the lint
//! here is correct rather than truly publishing 26 internal ECS plumbing types.
#![allow(private_interfaces)]
// Same rationale for these two: promoting the module tree to `pub` surfaced a
// handful of `pub fn from_str(...)` discriminator parsers (tsy_origin,
// tsy_container, mineral, alchemy, npc, combat, supply_coffin, …) and registry
// `len()` accessors (zhenfa) that were always crate-internal. They are not a
// cross-crate API; renaming/implementing `FromStr` or adding `is_empty` across a
// dozen unrelated modules would be unrelated churn for the §8.1 #10 bench work.
#![allow(clippy::should_implement_trait, clippy::len_without_is_empty)]

#[allow(dead_code)]
pub mod alchemy;
pub mod armor;
pub mod audio;
pub mod botany;
pub mod cmd;
pub mod coffin;
pub mod combat;
// craft：plan-craft-v1 P0+P1 通用手搓底盘。register() 注入 5 示例配方 + resources +
// events；P2/P3（client UI + agent narration + 三渠道 hook）由 plan vN+1 接入。
// 当前未在 Update systems 内消费 CraftStartedEvent / CraftCompletedEvent，
// 等 P2/P3 接入前保留 #[allow(dead_code)]。
#[allow(dead_code)]
pub mod craft;
#[allow(dead_code)]
pub mod cultivation;
// dandao: register() 接入 run_server（plan-dandao-runtime-wiring-v1 P0）；
// boss/boss_ai/catalyst_furnace 等子模块 P4 前仍有 dead items，保留 allow。
#[allow(dead_code)]
pub mod dandao;
#[allow(dead_code)]
pub mod death_lifecycle;
#[allow(dead_code)]
pub mod economy;
#[allow(dead_code)]
pub mod fauna;
#[allow(dead_code)]
pub mod forge;
#[allow(dead_code)]
pub mod gathering;
// identity：P0 锁定数据模型 + persistence；P1 起 /identity slash / consumer / scorer
// 等会逐步消费这些 API，初期保留 #[allow(dead_code)]，每个 P 接入后再收口。
#[allow(dead_code)]
pub mod identity;
pub mod inventory;
#[allow(dead_code)]
pub mod lingtian;
// mineral：M3 注册 MineralRegistry + MineralOreIndex + DiggingEvent listener；
// M2 worldgen 接入前 OreIndex 始终空，listener 对所有 block 静默 no-op。
#[allow(dead_code)]
pub mod mineral;
#[allow(dead_code)]
pub mod mob;
pub mod movement;
pub mod network;
pub mod npc;
pub mod persistence;
pub mod player;
#[allow(dead_code)]
pub mod qi_physics;
// preview：worldgen-snapshot harness 用的 server-side teleport hook。仅在
// BONG_PREVIEW_MODE=1 env 下激活实际 system；register() 一定会注册 event 类型
// 让 chat_collector 编译通过。
pub mod preview;
#[allow(dead_code)]
pub mod schema;
pub mod shader;
pub mod skin;
pub mod social;
pub mod spiritwood;
// supply_coffin：plan-supply-coffin-v1 — 巨剑沧海物资棺。
// register() 注入 SupplyCoffinRegistry resource + interact/refresh systems +
// SupplyCoffinOpened event。
pub mod supply_coffin;
#[cfg(test)]
mod test_coverage_guards;
// shelflife：M3a 注册 DecayProfileRegistry resource；compute_* / container_* 等
// 辅助仍未被 system 调用（M5 消费侧接入前）— 故保留 #[allow(dead_code)]。
#[allow(dead_code)]
pub mod shelflife;
pub mod skill;
#[allow(dead_code)]
pub mod sword_path;
// time: 节拍常量（MILLIS_PER_TICK 等），plan-sou-da-che-v1 #556 引入，被 network/combat
// 多模块 crate::time:: 引用 —— P7 lib 化时纳入 lib crate 模块树（合 main #556 解冲突）。
pub mod time;
#[allow(dead_code)]
pub mod tools;
#[allow(dead_code)]
pub mod tribulation;
pub mod world;
#[allow(dead_code)]
pub mod worldgen;
pub mod zhenfa;
#[allow(dead_code)]
pub mod zhenfa_hooks;
