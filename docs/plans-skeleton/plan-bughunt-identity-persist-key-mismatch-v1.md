# BugHunt: 身份持久化主键漂移——改名/识破标记/声望桥接写了但重连读不回来

## Bug 摘要

**严重度：high**（skeptic 判定 unchanged，未被调整）。

玩家 join 时唯一的 `PlayerIdentities` 加载点 `attach_identity_bundle_to_joined_clients`（`server/src/identity/mod.rs:307`）用 `canonical_player_id(username_str)`（`format!("offline:{username}")`，扁平、不带角色后缀）当 SQLite 查询主键；但 `identity/command.rs`（`/identity rename|new|switch`）、`identity/revealed.rs`（dugu 等流派被识破留下的永久 `RevealedTag`）、`social/mod.rs`（声望→身份桥接）三条独立写路径全部用 `Lifecycle.character_id` 当 key——而 `Lifecycle.character_id` 由 `combat/mod.rs:108-115` 生成，一旦玩家第一次落盘过（几乎必然，见下），恒为 `player_character_id(username, current_char_id)` = `offline:{username}:{uuid}`（带 UUID 后缀）。写 key 与读 key 从第二次及以后的会话开始永久不一致，导致每一次身份改名 / 永久识破 tag / 声望同步在重连后全部静默消失、被 `PlayerIdentities::with_default` 覆盖回默认状态。

## 实际游玩体验影响

玩家 `/identity rename` 改名后重连，名字打回原始 MC username；用暗器/独孤等流派招式被目击留下的**永久**（`is_permanent() == true`）身份识破标记，重连后凭空消失，等于白白暴露过身份却没留下代价；结契背盟、宗门背叛、越级全力击杀等声望事件同步到 active identity 后，重连同样被冲掉，玩家看到的"当前身份"声望分数和 NPC 反应/通缉/交易门禁读到的分数长期不一致。这不是极端边界情况——**任何玩家的第二次及以后登录**都会踩到，因为 `player_core.current_char_id` 在玩家第一次存档（正常游玩几乎瞬间发生）时就已经生成并写死。

## 证据定位

- `server/src/identity/mod.rs:307`：`attach_identity_bundle_to_joined_clients` 用 `let char_id = canonical_player_id(username_str);` 作为 `identity_db::load_player_identities(settings, &char_id)` 的 key（唯一的 join 加载点）。
- `server/src/player/state.rs:319-321`：`canonical_player_id(username)` = `format!("offline:{username}")`，扁平、不带角色后缀。
- `server/src/player/state.rs:323-329`：`player_character_id(username, current_char_id)` = `format!("{}:{current_char_id}", canonical_player_id(username))`（`current_char_id` 非空时带 UUID 后缀）。
- `server/src/combat/mod.rs:108-115`：`attach_combat_bundle_to_joined_clients` 用 `load_current_character_id(...).map(|id| player_character_id(username, &id)).unwrap_or_else(|| canonical_player_id(username))` 生成 `Lifecycle.character_id`——一旦 `current_char_id` 非空即恒为带后缀形态。
- `server/src/player/state.rs:2141-2149`：`current_char_id` 在玩家首次落盘（`INSERT ... ON CONFLICT(username) DO UPDATE`，UPSERT 逻辑）时若不存在则 `Uuid::now_v7()` 生成并写入 `player_core.current_char_id`——正常游玩第一次 autosave/断线即触发，此后恒非空。
- `server/src/identity/command.rs:333`：`handle_identity_command` 用 `let char_id = lifecycle.character_id.as_str();`；`/identity new`（L366）、`/identity switch`（L394）、`/identity rename`（L409）三条命令的 `save_identities(...)` 调用全部以此为 key。
- `server/src/identity/revealed.rs:88-99`：`consume_revealed_event<E: RevealedEvent>` 用 `let char_id = lifecycle.character_id.clone();` 调 `identity_db::save_player_identities(settings, &char_id, &identities)`；对应事件如 `identity/revealed.rs:64` `DuguRevealedEvent::is_permanent() -> true`——即这条永久标记也踩这个 key。
- `server/src/social/mod.rs:1497`：`apply_social_renown_deltas` 用 `players.iter_mut().find(|(lifecycle, _, _)| lifecycle.character_id == event.char_id)` 定位在线玩家，`event.char_id` 与 `lifecycle.character_id` 在同一行被断言相等——证明声望桥接写路径（L1514、L1580 两处 `identity_db::save_player_identities`）同样用的是带后缀 rotating id。
- `server/src/persistence/identity.rs:40-51`：`player_identities` 表 `char_id TEXT PRIMARY KEY`；`load_player_identities`（L98-124）按 `WHERE char_id = ?1` 精确匹配，无任何 fallback / 前缀匹配 / 迁移逻辑——key 对不上就是 `Ok(None)`。
- `server/src/persistence/identity.rs:55` 文档注释仍写着「`char_id` 用 `canonical_player_id` 计算（`offline:<username>`）」——这条注释本身就是过期契约的化石证据，写路径早已改用 rotating id 但没人回来同步这行注释和 `identity/mod.rs` 的加载逻辑。
- `git show --stat 6891a3a6a`（"修复死亡生命周期 review 问题"，2026-04-27）：改了 `combat/mod.rs`、`player/state.rs` 等文件，把 `Lifecycle.character_id` 切到 rotating per-character id，但未触及 `identity/mod.rs`、`identity/command.rs`、`identity/revealed.rs`、`social/mod.rs` 的 key 派生逻辑——四处写路径与 join 加载路径自此分叉。
- `server/src/identity/mod.rs:336-390`（`mod tests`）：现有单测（`identity_id_default_is_zero`、`with_default_uses_mc_username` 等）全部是纯函数级断言，没有一条用真实 `combat::attach_combat_bundle_to_joined_clients` 生成的 rotating id 做 join→mutate→rejoin 端到端回归。

## 触发路径

1. 玩家首次进服，`player_core.current_char_id` 为空，`identity/mod.rs:307` 用 `offline:{username}` 加载/创建默认身份；同 session 内 `combat/mod.rs` 因 `current_char_id` 尚未落盘（首次 autosave 前的极窄窗口），`Lifecycle.character_id` 也暂时等于 `offline:{username}`——两边碰巧一致，bug 不显现。
2. 玩家断线或首次 autosave 触发 `persist_player_slices_in_sqlite`（`player/state.rs:2141-2149`），`current_char_id` 首次生成为 UUID 并写入 `player_core`。
3. 之后任何一次操作——`/identity rename`（`identity/command.rs:409`）、被 dugu 等流派识破留下永久 `RevealedTag`（`identity/revealed.rs:88-99`）、结契背盟/宗门背叛/越级全力击杀等触发的声望桥接（`social/mod.rs:1497-1524`）——都以 `Lifecycle.character_id` = `offline:{username}:{uuid}` 为 key 写入 `player_identities` 表。
4. 玩家重新连接。`attach_identity_bundle_to_joined_clients`（`identity/mod.rs:307`）仍用扁平 `offline:{username}` 去查表，命中不到步骤 3 写的那一行（`load_player_identities` 精确匹配 PK，无 fallback），`loaded` 为 `None`。
5. 回退到 `PlayerIdentities::with_default(username_str, now_tick)`——改名、永久识破标记、声望同步全部静默清零，玩家看到自己"变回了默认身份"。

## 反方审查记录

- 第一轮质疑：
  - 是否只是理论推导、实际不可达？——查 `/identity` 命令定义（`identity/command.rs:48`）不在 CLAUDE.md dev-only 命令表内，是普通 brigadier 命令；dugu 识破、声望事件均来自正常战斗/社交玩法系统触发，不需要任何 dev 命令或崩溃时序窗口。
  - 是否存在某种 join 时的 key 迁移/兼容读取，只是没读到？——通读 `identity/mod.rs` 全文和 `persistence/identity.rs` 的 `load_player_identities`，确认唯一加载路径就是精确 PK 匹配，无 fallback。
  - 是否与 combat/cultivation 模块共享同一套 key 派生、只是我看错了 combat 那份？——对照 `combat/mod.rs:108-115`（`attach_combat_bundle_to_joined_clients`）逐行核对，确认 combat 侧用的是 `load_current_character_id + player_character_id` 的完整 rotating 派生，identity 侧确实独立走了简化版 `canonical_player_id`，两者未来自同一 helper。
  - 初裁：倾向通过，但要求补第二轮证据链闭合"写路径确实统一用 rotating id"。
- 第二轮补证：
  - 逐一核对三条写路径（`identity/command.rs:333`、`identity/revealed.rs:88`、`social/mod.rs:1497`）均取 `lifecycle.character_id`，且 `social/mod.rs:1497` 那行 `find(|(lifecycle, _, _)| lifecycle.character_id == event.char_id)` 本身就是"两者应相等"的断言，构成决定性证据。
  - 用 `git log -S` 定位到分叉起点 `6891a3a6a`（2026-04-27，死亡生命周期 review 修复）：该 commit 把 combat/cultivation 切到 rotating id，但未同步 identity 四处 key 派生点——是一次"改了一半模块、漏了另一半"的历史事故，不是设计如此。
  - 查重：`gh`/文档核对发现两个相邻主题的 plan——骨架 `plan-bughunt-k2-identity-social-renown-bridge-v1` 和 active `plan-social-renown-identity-bridge-v1`——都描述的是 `SocialRenownDeltaEvent` **写侧**（是否会同步进 `PlayerIdentities.active().renown`）缺失的问题；但实读当前 `social/mod.rs:1460-1533` 发现该写侧桥接已经由 `b43ba4b80`（"修复 social renown 身份桥接"）实现，两份 plan 文档本身已滞后于代码。无论如何，这两份 plan 处理的都是"写不写得进去"，完全没有触及本 bug——"join 时读哪个 key"；即便声望写路径再完善，join 时 `identity/mod.rs:307` 用错 key 依旧会让所有写入在重连后读不到。二者互不冲突、不重复。`plan-bughunt-heiwushi-dormant-identity-loss-v1` 是 NPC 休眠快照丢身份组件的另一个问题（boss/NPC 虚拟化路径），也与本 bug（玩家 `PlayerIdentities` 主键）无关。
  - 让步：目前是源码路径静态复现 + 精确 PK 匹配逻辑推导，尚未跑一条真实 join→mutate→rejoin 的集成测试实锤（这也是本 plan §验收测试计划里补的第一件事）。
  - 终裁：通过。这是缺 key 统一，不是需要新设计跨会话身份模型。

主循环复核：已亲读关键行确认。

## Skeleton Fix Plan

- [ ] 在 `server/src/identity/mod.rs` 的 `attach_identity_bundle_to_joined_clients` 中，把加载 key 的计算方式从裸 `canonical_player_id(username_str)`（L307）改为与 `combat::attach_combat_bundle_to_joined_clients`（`combat/mod.rs:108-115`）完全一致的解析逻辑：`load_current_character_id(persistence, username) -> Option<String>`，命中则 `player_character_id(username, &current_char_id)`，否则回退 `canonical_player_id(username)`。
- [ ] 确认/补齐 `load_current_character_id` 的可见性与依赖注入：`attach_identity_bundle_to_joined_clients` 目前只依赖 `Option<Res<PersistenceSettings>>`，需要按 combat 模块同款方式引入 `player::state::load_current_character_id`（如函数尚非 `pub(crate)`/`pub`，扩大可见性而非在 identity 模块另起一份重复实现）。
- [ ] 系统调度顺序检查：确保 `attach_identity_bundle_to_joined_clients` 在读取 `current_char_id` 时，`player_core` 表里对应行的写入（如果同 tick 有并发的玩家状态首次持久化）不会造成读时序竞态——若 combat 侧已有解决方案（例如都在 join 早期同一阶段跑、`current_char_id` 在更早的 `load_player_state` 路径已确定），直接复用同一时序假设，不要另造一套。
- [ ] `identity_db` 的 `save_player_identities` / `load_player_identities` 函数级文档注释（`persistence/identity.rs:55` 附近）同步更正，去掉"`char_id` 用 `canonical_player_id` 计算"这句过期描述，改为准确描述实际使用的是 rotating `Lifecycle.character_id`。
- [ ] 不新增第二套"迁移读取"（例如"先查带后缀 key，查不到再查扁平 key 自动搬迁"）当长期方案——这是掩盖 key 不统一的补丁式修复，正确做法是从根上统一 join 读 key 与三条写 key 的派生方式，让两者天然相等。
- [ ] 为 `identity` 模块补一条真正端到端的 join→mutate→模拟重连→再次 join 回归测试：构造一个真实 `current_char_id`（如经 `Uuid::now_v7()` 生成），先跑一次 join（生成 `Lifecycle.character_id` 带后缀），再走 `/identity rename` 或 `consume_revealed_event` 写一次身份变更并持久化，然后模拟二次 join（重新调用 `attach_identity_bundle_to_joined_clients`），断言读回的 `PlayerIdentities` 命中步骤二写入的数据而不是回退到 `with_default`。
- [ ] 本 bug 不涉及真元/灵气流动，无需接入 `qi_physics::ledger`；本 bug 也不涉及 C2S 请求门禁（纯 server 内部 join-time hydrate 逻辑），无需 server gate 权威性讨论——两条 CLAUDE.md 硬约束在此 plan 范围内均不适用，仅作确认说明。

## 验收测试计划

- **happy path（server cargo test，`identity` 模块）**：构造一名"老玩家"（`player_core.current_char_id` 已存在、非空 UUID），走 `attach_combat_bundle_to_joined_clients` 风格的 join 生成 `Lifecycle{character_id: "offline:alice:<uuid>"}`，随后调用 `apply_rename`/`save_identities` 落盘一次改名；模拟断线重连再次调用（修复后的）`attach_identity_bundle_to_joined_clients`，断言返回的 `PlayerIdentities.active().display_name` 等于重连前设置的新名字，而不是原始 username。
- **边界 1（首次 join，`current_char_id` 为空）**：`load_current_character_id` 返回 `None` 时，加载 key 应回退为 `canonical_player_id(username)`，与首次 join 时尚未生成 `Lifecycle.character_id` 后缀的窗口保持一致；断言此时仍能正确创建/加载 `PlayerIdentities::with_default`。
- **边界 2（已有旧数据：写在扁平 key 下的历史行）**：构造一条已经存在于 `player_identities` 表、key 为扁平 `offline:{username}`（模拟修复前遗留的"从未改过名"老玩家)的记录；断言修复后若该玩家 `current_char_id` 也已生成（回归为 rotating key 查询），扁平旧行**不会**被自动命中——据此在验收阶段同时产出一份"迁移前后 key 对照"说明（见风险节），而不是让测试悄悄通过掩盖迁移缺口。
- **错误分支（persistence 不可用）**：`Option<Res<PersistenceSettings>>` 为 `None` 时（无持久化后端），join 应直接走 `PlayerIdentities::with_default`，不 panic、不因为缺少 `load_current_character_id` 依赖而报错。
- **状态转换（rename → revealed → renown 三写路径交叉）**：单条集成测试内连续触发 `/identity rename`（`identity/command.rs`）→ dugu 流派 `consume_revealed_event`（`identity/revealed.rs`，`is_permanent() == true`）→ `SocialRenownDeltaEvent` 桥接（`social/mod.rs::apply_social_renown_deltas`），三次写入均使用同一个 rotating `char_id`；模拟重连后一次性断言三项状态（display_name、`RevealedTag` 存在、`active().renown` 数值）全部读回正确，而不是逐项单独测试掩盖交叉场景下的漂移。
- **回归测试位置**：优先加在 `server/src/identity/mod.rs` 的 `mod tests`（or 新建 `server/src/identity/integration_tests.rs`，视仓库既有集成测试组织习惯而定），需要真实调用 `player::state::{canonical_player_id, player_character_id, load_current_character_id}` 与 `persistence::identity::{save_player_identities, load_player_identities}`，不能只 mock 掉 key 计算过程本身（那样测的是"我以为它对"而不是"它真的对"）。

## 风险

- **线上已有脏数据**：修复上线前，任何在本 bug 存在期间产生的"写在带后缀 rotating key 下但从未被读回过"的 `player_identities` 行会在修复后突然被读到——这其实是**修复行为**（找回本该属于玩家的数据），但如果同一玩家期间又在扁平 key 下被动创建过一份 `with_default` 默认身份（例如多次重连都在原 bug 状态下运行，产生了扁平 key 下的默认记录被后续 `/identity` 操作污染），修复后可能出现"该读哪一条"的合并冲突，需要在上线前跑一次数据审计脚本，列出所有同一 username 前缀下同时存在扁平 key 和带后缀 key 两行的账号，人工决定合并策略（默认应以带后缀 rotating key 那份为准，因为它是修复后唯一权威 key）。
- **不应顺手做自动迁移合并**：修复本身只改"join 时用哪个 key 查"，不应该在这个 PR 里顺带写一个"自动把扁平 key 数据搬到 rotating key"的迁移脚本——迁移策略（覆盖 vs 保留旧 vs 人工审计）超出这个 gate bug 的最小修复范围，属于需要人工确认的一次性数据运维动作，不要在 PR 内静默执行。
- **`load_current_character_id` 依赖的时序假设**：如果 identity join 系统与 combat/cultivation join 系统的 Bevy schedule 顺序不同（例如 identity 系统跑在 `current_char_id` 尚未被其他系统首次持久化之前），复用 combat 同款解析逻辑可能仍会在极窄的"从未存过档的全新账号第一 tick"内出现 key 不一致——但这与首次 join 时 combat 侧本身也一致回退到 `canonical_player_id` 的窗口相同，不是本修复引入的新问题，只需在测试里覆盖这个边界（见验收测试"边界 1"）即可，无需额外系统排序改动。
- **不要在这个 plan 顺带扩大 `identity` 模块职责**：本 bug 修复范围严格限定为"统一 join 读 key 与三条写 key 的派生逻辑"，不应借机重构 `PlayerIdentities` 的 schema、新增角色切换相关的产品功能，或者改变 `/identity` 命令的用户可见行为——那些是独立的功能类 plan（若有需要应走 `/consume-plan` 而非 bughunt 修复通道）。
