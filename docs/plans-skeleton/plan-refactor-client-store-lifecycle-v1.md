# plan-refactor-client-store-lifecycle-v1 — Client 状态 Store 统一断线生命周期（重构轨 R2）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：把 client 侧 108 个各自为政的静态 Store 收敛到统一 `SessionScopedStore` 接口 + 自动登记清理，让"断线残留/跨会话串味"这一整类 bug（现存 14+ 份 plan）在架构上不可能再发生。

## 现状证据（2026-07-27 侦察）

- 全仓 `*Store.java` 108 个，定义 `clearOnDisconnect()` 的仅 28 个，被 `BongNetworkHandler.clearClientStateOnDisconnect()`（`BongNetworkHandler.java:1119-1200`）手工清单实际调用的仅 25 个；清单行内注释就是历史补漏记录（TiandaoPresence/DroppedItem/DuguV2Hud 各补过一次），结构性反复漏项。
- 裸奔实例：`processing/state/FreshnessStore.java`（只有 `clearForTests`）、`combat/store/` 一整排（Wounds/Vortex/Terminate/DeathState/DuguPoison/Tribulation/AscensionQuota）只有 `resetForTests()`，且被 `BongHudOrchestrator` 直接消费。
- 清理动作方法名三种混用（`clearOnDisconnect`/`clear`/`reset`），无共同接口。
- 会话状态机本身是干净的：`ui/ClientConnectionStatusStore`（initialize/activate/invalidateSession）——问题只在"业务 store 没挂上这个生命周期"。
- 既有守护测试 `BongNetworkHandlerTest.java:560-575` 用源码字符串扫描断言 DISCONNECT 路由，可扩展为全量强制。

## 接入面

- **进料**：`ClientPlayConnectionEvents.DISCONNECT`（既有唯一权威入口）、`ClientConnectionStatusStore` 会话状态机。
- **出料**：所有 HUD planner / Screen / tooltip 读到的 store 快照保证是本会话的。
- **共享类型**：新 `SessionScopedStore` 接口（单一 `clearOnDisconnect()` 语义）+ 中央注册表（store 构造时自注册或静态登记）；不改各 store 的业务读写 API。
- **跨仓库契约**：零 wire 改动；loop 音效清理对齐 `feedback_audio_loop_lifecycle`（硬停连延迟层一起哑）。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：108 个 store 分类普查（会话态/持久配置态/纯常量表），冻结接口与登记机制；定强制手段（测试期反射/源码扫描断言"所有会话态 store 都实现并登记"，新增漏登记即红）。
- ⬜ P1 框架落地：`SessionScopedStore` + 注册表 + `clearClientStateOnDisconnect()` 改为遍历注册表；现有 25 个在册 store 平移，行为不变。
- ⬜ P2 裸奔 store 收编：FreshnessStore、combat/store/ 全排、动画层缓存（playeranim reconnect stale layer）、循环音效 loop 生命周期挂断线硬停。
- ⬜ P3 全量强制 + 删旧：三种清理方法名统一、手工清单删除、强制测试上线；跨会话残留类 skeleton 逐个复测确认不可复现。
- ⬜ P4 bot 验收 + 吸收 plan 批量归档。

## 吸收清单（active 13 + skeleton 若干；短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：alchemy-ui-session-stale、breakthrough-billboard-session-leak、client-freshness-store-session-stale、forge-ui-session-stale、full-power-charging-session-bleed、lingtian-session-disconnect-ui、perception-edge-session-leak、playeranim-reconnect-stale-layer、poison-trait-hud-disconnect、spirittreasure-session-leak、tsy-extract-disconnect-stale、woliu-vortex-disconnect-residue、yidao-hud-disconnect-bleed。
skeleton：niche-guardian-cross-session-leak、ambient-zone-audio-stale-anchor（音效锚点/loop 生命周期部分）、zone-environment-audio-loop-fallback（loop 回退的生命周期部分；音效映射数据修复独立保留）。

## 文件所有权与边界

- 独占：全部 `*Store.java` 的生命周期接口与登记、`BongNetworkHandler.java` 的 `clearClientStateOnDisconnect` 区段。
- 不碰：`BongNetworkHandler.register()` 的 channel 注册区（R6 域，同文件分区段协作，两轨 merge 前互相 fetch）；Screen 结构（R7 域）；store 的业务字段语义。
- 依赖：无前置，Wave 0 即可动工。R7/R9 依赖本轨接口，先于它们合入。

## bot 验收场景

bot 是协议级客户端，测不了 client 内存——本轨主验收是 client 单测（注册表强制扫描 + 每类 store 断线清理 pin 测试），bot 侧配合场景：
1. `reconnect_state_freshness`：bot 断线重连后 server 重发的首包快照集完整（联动 R6 join 首包契约），保证"清干净之后能重新灌满"。

## 开放问题（pre-P0 收口）

1. 注册机制选型：构造器自注册（运行时） vs 静态登记表 + 源码扫描强制（现有测试风格）？倾向后者（无 Fabric 运行时依赖）。
2. `resetForTests` 与生产清理是否合一（删 test-only 方法）？
