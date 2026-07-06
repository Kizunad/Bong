# plan-bughunt-skillbar-cast-source-drift（skeleton）

> BugHunt D7 / client-combat。仅记录高置信 bug skeleton，不消费、不归档、不改代码。

## Bug 摘要

`cast_sync` S2C payload 没有携带 cast source，客户端 `CastSyncHandler.sourceFor()` 只能从当前本地 `CastStateStore` 猜测来源。玩家在技能栏招式 A 读条中切到技能栏招式 B，或技能栏招式自然完成后收到服务端 complete 回包时，服务端已经知道这是 `CastSource::SkillBar`，但客户端会把回包重标成 `QUICK_SLOT`，导致技能栏读条/完成/中断反馈跑到上排 quick-use 栏，或在上排无绑定时看起来直接消失。

## 对实际游玩体验的影响

玩家连续按两个 1-9 技能栏招式时，下排技能栏本应显示新招式 B 的读条；实际会被旧招式 A 的 interrupt 回包污染成上排 quick-use 来源，随后 B 的 casting 回包也继续显示在上排。若同 slot 上排没有物品快捷绑定，玩家会看到技能栏读条突然消失；若上排有物品绑定，则会误以为物品快捷栏在施法。技能本身可能仍在服务端执行，但玩家失去“当前哪一招正在读条 / 是否完成 / 是否中断”的关键战斗反馈。

## 证据定位

- `client/src/main/java/com/bong/client/network/CastSyncHandler.java:35` 先调用 `sourceFor(slot)`，再构造 `CastState`；`sourceFor()` 只在当前仍 `CASTING`、同 slot、且当前 source 为 `SKILL_BAR` 时返回 `SKILL_BAR`，否则默认 `QUICK_SLOT`（同文件 `:97`-`:102`）。
- `client/src/main/java/com/bong/client/combat/SkillBarKeyRouter.java:53`-`:60` 在切换技能栏 slot 时，先本地 interrupt 当前 cast，再 `beginSkillBarCast()` 开启新 slot，本地状态会短暂正确地变成 B / `SKILL_BAR`。
- `server/src/network/client_request_handler.rs:10206`-`:10215` 收到新技能栏施法时会先 `cancel_previous_cast()`；`server/src/network/client_request_handler.rs:10724`-`:10736` 对旧 A 推 `cast_sync(Interrupt)`；随后 `server/src/network/client_request_handler.rs:10669`-`:10691` 对新 B 推 `cast_sync(Casting)`。
- `client/src/main/java/com/bong/client/hud/QuickBarHudPlanner.java:157`-`:160` 只把 `QUICK_SLOT` cast bar 画到上排；`:234`-`:236` 只把 `SKILL_BAR` cast bar 画到下排。
- `server/src/schema/combat_hud.rs:95`-`:102` 与 `proto/bong/envelope.proto:1511`-`:1517` 的 `CastSync` 只有 `phase/slot/duration_ms/started_at_ms/outcome`，没有 source；`server/src/schema/proto_convert.rs:1972`-`:1979` 也只转换这些字段。
- 现有 `client/src/test/java/com/bong/client/network/CastSyncHandlerTest.java:80`-`:92` 只保护“旧 skillbar 终态不要污染后续 quick-slot cast”，没有覆盖“服务端回包本身属于 skillbar cast 却被误判为 quick-slot”。

## 触发路径

1. 下排技能栏 slot 1 绑定招式 A，slot 2 绑定招式 B。
2. 玩家按 slot 1，客户端本地进入 A / `SKILL_BAR` casting，并向服务端发送 `skill_bar_cast(slot=1)`。
3. A 读条未结束时，玩家按 slot 2；客户端本地 interrupt A 后立即 `beginSkillBarCast(2)`，此时本地状态是 B / `SKILL_BAR` casting。
4. 服务端处理 slot 2：先取消旧 A，推 `cast_sync(Interrupt, slot=1)`；再启动 B，推 `cast_sync(Casting, slot=2)`。
5. 客户端处理旧 A interrupt 时，`sourceFor(1)` 看到当前是 slot 2 casting，slot 不同，于是返回 `QUICK_SLOT` 并覆盖全局 cast state。
6. 客户端处理 B casting 时，当前已是 interrupt，不再 `isCasting()`，`sourceFor(2)` 继续返回 `QUICK_SLOT`。
7. HUD 按 source 分流，B 的读条从下排技能栏迁到上排 quick-use 行，或直接不可见。

同型路径：技能栏 cast 本地自然 `complete()` 后，服务端 `complete` 回包到达；由于当前不再是 `CASTING`，`sourceFor()` 默认 `QUICK_SLOT`，完成反馈也会被重标。

## 反方审查记录

Round 1：反方尝试推翻触发链，结论为成立。它确认 `SkillBarKeyRouter` 的本地 B / `SKILL_BAR` casting、服务端旧 A interrupt 再新 B casting 的顺序、以及 `sourceFor()` 的默认 `QUICK_SLOT` 会串成真实错位；未发现 #987/#997/#1002/#1012/#1018/#1016 覆盖该问题。

Round 2：反方专攻修复风险，结论为通过。现有 `completedSkillBarCastDoesNotRelabelNextQuickSlotCast` 不是反证，它保护的是后续 quick-slot cast 不被旧 skillbar 终态污染；本 bug 是服务端权威 skillbar 回包缺 source 后被误判。反方建议不要继续靠本地配置启发式猜测，因为上下两排可共享同一 index，最稳修复是扩 `CastSyncV1/proto` 增加 source，并保留旧 payload fallback。

## Skeleton Fix Plan

- [ ] P0 红测复现：在 `CastSyncHandlerTest` 中新增 A skillbar -> B skillbar 的回包序列，断言最终 B 保持 `CastState.Source.SKILL_BAR`；新增 skillbar complete 回包保持 `SKILL_BAR`；保留 quick-slot 同 slot 后续 cast 不被误标 skillbar 的保护用例。
- [ ] P1 协议补源：给 `CastSyncV1` / proto `CastSync` / TS schema（如有镜像）增加 `source` 字段，取值区分 `quick_slot` 与 `skill_bar`；更新 proto 转换和 JSON bridge。
- [ ] P2 服务端权威填充：所有 `push_cast_sync` 调用路径都传入真实 source。`Casting` 存在时从 `Casting.source` 填；施放前拒绝路径按请求入口填（`use_quick_slot` 为 quick slot，`skill_bar_cast` 为 skill bar）。
- [ ] P3 客户端消费：`CastSyncHandler` 优先使用 payload source；只有旧 payload 缺 source 时才走兼容 fallback。fallback 不能覆盖新协议的权威 source。
- [ ] P4 HUD 回归：补 `QuickBarHudPlannerTest`，断言 skillbar source 的 cast bar 只出现在下排，quick-slot source 的 cast bar 只出现在上排。

## 验收测试计划

- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build`
- 若改 schema/proto：按仓库现有流程重建相关 schema/proto 产物，并跑覆盖 `ProtoServerDataBridgeTest` / schema roundtrip 的测试。
- 手动 UI 验证：在 client 中连续按两个技能栏招式，确认下排技能栏显示新招式读条；上排 quick-use 行不出现该读条；读条完成和中断反馈仍留在下排。

## 风险

- 协议增加字段会影响 server/client/schema/proto 多端镜像，必须保持旧 payload 兼容，避免线上混版本客户端把 `cast_sync` 整条丢掉。
- 施放前拒绝没有 `Casting` component，source 需要由请求入口显式传入；不能再次退回不可靠的 slot 猜测。
- 同一 index 可同时存在上排 quick item 和下排 skillbar skill，本地配置不能作为权威 source。
- 修复只应改 cast source 归属，不应改变技能 resolver、冷却、A/V 事件发射或 PlayerAnimator 动画资源。
