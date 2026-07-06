# plan-bughunt-af-target-info-anonymous-name-leak-v1（骨架）

> **骨架（草案）**。一句话主题：`inspect / social / hud / state` 主路径发现 1 个高置信真 bug：**匿名玩家头顶名牌虽已隐藏，但 `TargetInfo` 顶部 HUD 仍会在一次右键/攻击后泄漏其真实名字 5 秒**。这条链路直接绕过 `plan-social-v1` 的匿名设计，对 PvP/尾随/试探交互都有明确玩法后果。

> 立项动机：本轮限定扫描 `inspect/preview/social-hud/state` 主路径，并排除已立项题（trade bundle 少发货 / sparring invite hijack / trade gate / season stale client / tide_sky 漏接等）。本题落点集中、证据链短、玩家体感强，适合 skeleton-only 立项。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 匿名玩家被 `TargetInfo` HUD 反查真名 | fix_pr | ⬜ |

## P0 — 匿名玩家被 `TargetInfo` HUD 反查真名

- **#1 major（fix_pr）**：匿名系统只挡住了**头顶名牌**，没有挡住**顶部 TargetInfo HUD**。
  - `client/src/main/java/com/bong/client/mixin/MixinEntityRenderer.java:17-35` 明确把远端玩家名牌显示门控到 `SocialStateStore.shouldShowRemoteNameTag(player.getUuidAsString(), playerName)`；`docs/finished_plans/plan-social-v1.md:292-296` 也把“client 端 name tag 默认隐藏 / server 下发 AnonymityPayload 决定显示”写成正式交付。
  - 但 `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java:41-68` 在**左键攻击**与**主手右键交互**后都会无条件 `TargetInfoStateStore.observeEntity(...)`。
  - `client/src/main/java/com/bong/client/hud/TargetInfoState.java:69-90` 处理 `PlayerEntity` 时，直接取 `living.getDisplayName().getString()` 作为 `displayName`，**完全不查询 `SocialStateStore`，也没有匿名兜底文案**；realm 还被硬编码成空串。
  - `client/src/main/java/com/bong/client/hud/TargetInfoHudPlanner.java:55-72` 会把这个 `displayName` 原样渲染到屏幕顶部；对玩家目标只是不画 HP/真元条，**并不会隐藏名字本身**。
  - 结果：匿名玩家虽然头顶没名牌，但只要被人点一下/打一下，顶部 HUD 就会显示其真实用户名并保持 `TargetInfoState.HOLD_MILLIS = 5000` 毫秒。匿名机制被一次交互直接绕过。

## 这个 bug 对实际游玩体验的影响

- 匿名玩家在遭遇战、尾随试探、切磋前试探时，本应只暴露“有人在这里”，现在却会被一次轻触直接暴露真实身份。
- 这让 `plan-social-v1` 的“默认匿名、暴露后才显名”规则失去实战意义；玩家不需要等 `social_exposure`、不用 inspect 面板，也不用任何高阶感知，就能从 HUD 读出真名。
- `docs/finished_plans/plan-hud-immersion-v2.md:5` 已明确写了“匿名系统（默认不显示名字）→ HUD 不应暴露他人太多信息”；当前实现和该约束正面冲突。

## 建议修法

- `TargetInfoState.fromEntity(PlayerEntity)` 改为复用 `SocialStateStore.shouldShowRemoteNameTag(player.getUuidAsString(), playerName)` 这条既有匿名判定，而不是直读 `living.getDisplayName()`。
- 未暴露时，`TargetInfo` 应降级成匿名文案（如“某修士”或其他已定匿名占位），并继续保持玩家目标不显示 HP/真元；不要在这条快速 HUD 上额外泄漏 realm。
- 已暴露/已知名的玩家仍可沿用现有显示逻辑，避免伤到正常熟人/盟友可见性。

## 测试抓手

- 补 `TargetInfoState` / `TargetInfoHudPlanner` 单测：
  - 匿名 remote：`SocialStateStore.replaceAnonymity(... anonymous=true ...)` 后，玩家目标 HUD **不得**出现真实名字。
  - 已暴露 remote：`anonymous=false` 时，HUD 正常显示名字。
  - pin `TargetInfoStateStore.observeEntity(PlayerEntity, now)` 走玩家分支，防未来只测 `TargetInfoState.create(...)` 这类绕开真实入口的假阳性。
- 现有 `client/src/test/java/com/bong/client/network/SocialServerDataHandlerTest.java:24-48` 只验证了**头顶名牌**门控；`TargetInfo` 路径目前零覆盖。

## 两轮反方裁决摘要

1. **反方第 1 轮**：也许匿名玩家的 `PlayerEntity.displayName` 已被 server 改写成化名，所以 `TargetInfo` 读到的不一定是真名。  
   **裁决**：证伪。全仓未发现任何把远端 `PlayerEntity` 名称改写为匿名占位的链路；现有匿名实现只有 `MixinEntityRenderer` 取消 `renderLabelIfPresent`，不是改名。
2. **反方第 2 轮**：也许 `plan-social-v1` 只要求隐藏头顶名牌，不要求隐藏顶部 HUD，所以这属于设计允许。  
   **裁决**：证伪。`plan-social-v1` 已把匿名作为正式玩法约束，且 `plan-hud-immersion-v2.md:5` 明写“HUD 不应暴露他人太多信息”；一次攻击/右键就泄漏真名，与“默认匿名、暴露后才显名”的设计目标冲突。

## 审计来源

bug-hunt 线程 AF（限定 worktree：`bughunt-loop-20260705-af`，范围：`inspect / preview / social-hud / state` 主路径）。候选链路先后排除了已修 bridge 枚举问题与已立项主题后，最终锁定 `TargetInfoStateStore -> TargetInfoState -> TargetInfoHudPlanner` 对匿名系统的旁路泄漏。结论：**real-on-main，player-facing，局部明确，可 fix_pr。**
