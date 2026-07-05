# plan-ash-spider-disguise-nametag-leak-v1（骨架）

> **骨架（草案）**。一句话主题：**拟态灰烬蛛伪装态名牌泄漏**。server 已明确把蛛设成 `NameVisible(false)` 以保证“伪装为灰烬方块”，但 client 的 `NpcNametagRenderer` 仍会基于 `bong:npc_metadata` 给它画出 `[妖兽·醒灵]` / `兽` 浮空标签，直接破坏伏击与留活追兵玩法。该题与已出的“拟态灰烬蛛缺伪装贴图”不同：那是**资源缺失**，本题是**渲染/反馈链绕过隐藏契约**。

## 复现路径

1. 进入死域边缘，触发拟态灰烬蛛自然刷新链：`server/src/mob/ash_spider.rs` 控制边缘/巢穴权重，`server/src/world/mob_spawn.rs:48-75` 把 `NaturalMobKind::AshSpider` 调度到 `spawn_ash_spider_npc_at(...)`。
2. 服务端生成时就把蛛置于 `SpiderDisguiseState::Disguised`，并显式写入 `NameVisible(false)`；证据见 `server/src/npc/spawn_spider.rs:4-7,43,68-69,162-179`。
3. client 侧同时走两条链：
   - 伪装渲染链：`bong:spider_disguise_enter` → `SpiderDisguiseHandler` → `FaunaModel` 切成灰烬方块贴图（这条链本身是通的）。
   - 名牌链：`server/src/network/npc_metadata.rs:123-185` 把所有 `With<NpcMarker>` 实体同步成 `NpcMetadata`；`NpcArchetype::Beast` 会被命名为 `妖兽·<境界>`，见 `server/src/network/npc_metadata.rs:343-357`。
4. 玩家靠近未暴起的拟态蛛：
   - 20 格内：client 画 `[妖兽·醒灵]`
   - 20-40 格：client 画 `兽`
   证据见 `client/src/main/java/com/bong/client/npc/NpcNametagRenderer.java:39-47,54-98,106-118`。
5. 结果是：灰烬方块外观上方悬浮着明确的敌对标签，玩家在蛛暴起前就能远距离识破。

## 根因链路

1. **server 契约是“隐藏名牌”**：`spawn_spider.rs` 注释、组件表和测试都把 `NameVisible(false)` 当作伪装成立条件（`server/src/npc/spawn_spider.rs:4-7,43,162-179`）。
2. **metadata 同步把 Beast 也当作近程可标注 NPC**：`emit_npc_metadata_payloads` 对所有 `With<NpcMarker>` 下发 `NpcMetadata`，没有排除 `NpcArchetype::Beast` 或伪装态（`server/src/network/npc_metadata.rs:123-185`）。
3. **client 自定义名牌完全绕开了 server 的隐藏位**：`NpcNametagRenderer` 在 `AFTER_ENTITIES` 遍历 `client.world.getEntities()`，只要 `NpcMetadataStore` 里有条目就画字；它既不检查 `entity.shouldRenderName()` / tracked `NameVisible`，也不检查 `SpiderDisguiseHandler.isDisguised(entityId)`（`client/src/main/java/com/bong/client/npc/NpcNametagRenderer.java:54-98`）。
4. **这与原 plan 验收直接冲突**：拟态蛛 plan 明确要求“Disguised 时 client nameplate 不显示”，见 `docs/finished_plans/plan-fauna-mimic-spider-v1.md:109-113`；后续 Finish Evidence 还写明实现已改为 `NameVisible(false)` 路线（同文件 `:147`）。

## 这个 bug 对实际游玩体验的影响

- 死域边缘的“看起来只是残灰方块，踩上去才暴起”被直接打穿；玩家能在 40 格外靠 `兽` 单字标签锁定埋伏点。
- `worldview` 与 `plan-fauna-mimic-spider-v1` 里“老玩家故意留活蛛阴追兵”的玩法失效，因为追兵同样会被悬浮标签提前剧透。
- 这不是纯视觉瑕疵，而是把一整个伏击/伪装生态位从“信息不对称玩法”降成“头顶写着答案的普通怪”。

## 影响面

- **已确认命中**：拟态灰烬蛛 `Disguised` 态。
- **高风险同类面**：任何未来继续依赖 `NpcMetadata + 自定义名牌`、同时又要求 `NameVisible(false)` 的伪装型实体，都会复用同一个断链面。
- **去重说明**：本题不重复最近已出的贴图缺失 / social renown / craft / tribulation / botany / dying elder / pseudo vein / preview pause / npc dormant / insight / extra hand / spirittreasure affinity desync。

## 修复建议

1. 以 **server 的 `NameVisible` 为单一真相**：`NpcNametagRenderer` 绘制前必须先尊重实体当前的 name-visible 状态，而不是“metadata 一到就必画”。
2. 防御性补闸：对 `SpiderDisguiseHandler.isDisguised(entityId)` 命中的蛛，直接短路不画名牌，避免后续再被别的 client overlay 绕开。
3. 补 client 回归测试：
   - `Disguised` 蛛存在 `NpcMetadata` 时仍不出名牌。
   - `Ambush` 后解除伪装，名牌才恢复。
   - Beast metadata 不得覆盖 `NameVisible(false)`。

## 反方裁决

### Round 1

- **反方论点**：`NpcMetadata` 本来就覆盖 Beast；给野兽画标签是有意的可读性设计，不算 bug。
- **驳回理由**：本题不是泛泛讨论“野兽要不要有标签”，而是 `spawn_spider.rs` 已把拟态蛛单独定义成 `NameVisible(false)`，且测试文案直接写“伪装效果依赖此”。当 server 为某个实体显式下发“隐藏名牌”时，client 不能再用第二套 overlay 把它重新暴露出来。

### Round 2

- **反方论点**：`NpcNametagRenderer` 是 client 自绘 overlay，不是 vanilla nameplate；既然不是同一系统，就不算违反“nameplate hidden”。
- **驳回理由**：从玩家视角看，它就是实体头顶的可读标签；而 `plan-fauna-mimic-spider-v1` 的验收文字写的是“Disguised 时 client nameplate 不显示”，关注的是**最终观感**，不是实现细节。只要头顶还能读到 `[妖兽·醒灵]` / `兽`，伪装就已经失败。

## 退化说明

- 当前会话没有可用的 subagent / delegate 工具可再开独立反方审查，本次按要求做了**人工两轮反方裁决**。
- 结论基于源码交叉取证与既有 plan/测试契约，未做源码修改。
