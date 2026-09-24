pub(crate) mod combat;
pub(crate) mod forge;
pub(crate) mod inventory;
/// 外置 Forge typed-dispatch 契约测试所需的最小公共接缝。
///
/// Forge 实现模块本身保持 crate 内可见；这里只暴露 extractor、dispatcher 与其
/// typed request，避免 `server/tests/**` 为契约 pin 而依赖整个内部实现树。
#[doc(hidden)]
pub mod forge_contract {
    pub use super::forge::{dispatch_forge_request, try_into_forge_request, ForgeRequest};
}
/// 外置 inventory typed-dispatch 契约测试所需的最小公共接缝。
#[doc(hidden)]
pub mod inventory_contract {
    pub use super::inventory::{
        dispatch_inventory_request, try_into_inventory_request, InventoryRequest,
    };
}
pub(crate) mod npc;
pub(crate) mod production;
pub(crate) mod scroll;
pub(crate) mod social;
pub(crate) mod world;

// The typed scroll extractor is crate-private production plumbing. Keep its
// small protocol contract test beside the client-request modules rather than
// adding a test-only public seam for an integration target.
#[cfg(test)]
mod tests;
