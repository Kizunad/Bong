pub mod faction_tint;
pub mod mineskin;
pub mod npc_skin_selector;
pub mod packet;
pub mod pool;

use std::path::PathBuf;

use valence::prelude::App;

pub use npc_skin_selector::{initial_age_ratio, select_npc_visual_profile, NpcVisualProfile};
pub use pool::{npc_uuid, NpcPlayerSkin, NpcSkinFallbackPolicy, SkinPool};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedSkin {
    pub value: String,
    pub signature: String,
    pub source: SkinSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SkinSource {
    MineSkinGenerate { uuid: String, timestamp: u64 },
    MineSkinRandom { hash: String },
    LocalPack { path: PathBuf },
}

pub fn register(app: &mut App) {
    faction_tint::register(app);
    pool::register(app);
}
