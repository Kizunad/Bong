# Bong · plan-faction-expansion-v1 · 骨架

**具名散修势力**——将当前 `FactionId { Attack, Defend, Neutral }` 三档占位升级为**有名字、有地盘、有头目、有历史背景**的具名散修帮派，给 worldview §十一 "散修江湖人来人往"的设定赋予实体。v1 先实装 3-5 个具名势力（enum 扩展 + 地盘 zone 绑定 + 领袖 NPC），为 `plan-faction-wars-v1` 的玩家参与战争奠定基础。

**来源**：`plan-faction-wars-v1.md`（skeleton）§反向被依赖 "plan-faction-expansion-v1（待立，具名散修势力 + 宗门系统）"

**交叉引用**：`plan-npc-ai-v1.md` ✅（big-brain Scorer/Action + FactionStore + FactionMembership）· `plan-npc-virtualize-v1.md` ✅（dormant SoA + 派系 zone 持久化）· `plan-social-v1.md` ✅（Renown + NPC 信誉度 baseline）· `plan-narrative-political-v1.md` ✅（feud/pact 事件：具名派系后才能有具名政治事件）· `plan-faction-wars-v1.md` ⬜（玩家参与战争，本 plan 硬前置）· `plan-qi-physics-v1.md` ✅（守恒律：派系地盘 zone 灵气流向由 qi_physics 管控）

**worldview 锚点**：
- **§十一:947-970 散修江湖**：NPC 没有灵龛，靠追踪高灵气浓度生存；派系 = 有共同利益/仇恨的散修临时联盟（"人来人往"说明派系流动性强，不是永久宗门）
- **§十一 匿名系统**：派系成员的具体身份可以隐藏；被识破加入某派系会改变 NPC 信誉度
- **§三:124 NPC 与玩家平等**：派系领袖 NPC 按与玩家相同的境界/真元规则运作，不是作弊的 boss
- **§九 交易生态**：具名势力垄断特定地盘的灵草/矿脉 → 控制资源供给链 → 产生交易博弈

**qi_physics 锚点**：
- 派系控制地盘（zone）后，`spirit_qi` 吸收率走 `qi_physics::regen_from_zone` + `FactionZoneBonus`（修正系数，不引入新常数，只调现有 regen multiplier）
- 派系领袖死亡走 `qi_physics::qi_release_to_zone`（继承现有路径）

**前置依赖**：
- `plan-npc-ai-v1` ✅ — FactionStore / FactionMembership / big-brain Scorer/Action
- `plan-npc-virtualize-v1` ✅ — dormant SoA（具名势力的 dormant 成员批量持久化）
- `plan-social-v1` ✅ — Renown + NPC 信誉度（玩家与具名势力的关系靠 Renown 建立）
- `plan-qi-physics-v1` ✅ — 守恒律底盘

**反向被依赖**：
- `plan-faction-wars-v1` ⬜ — 派系战争需要具名派系作为战争主体
- `plan-narrative-political-v1` ✅（已 finished）— feud/pact 叙事可升级为"具名派系 vs 具名派系"
- `plan-npc-virtualize-v3` ⬜ — dormant 批量战争推演的参与方是具名派系

---

## 接入面 Checklist

- **进料**：`npc::faction::FactionId`（扩展 enum）/ `npc::spawn::NpcTemplate`（领袖 NPC spawn）/ `worldgen` zone 坐标（地盘绑定）/ `plan-social-v1` Renown 字段
- **出料**：`FactionRegistry` Resource（具名势力 metadata: 名称/地盘/历史/关系矩阵）→ FactionStore；`NamedFactionLeader` Component（领袖 NPC entity 标记）；`bong:faction_state` Redis HASH（agent 可读派系势力数据）
- **共享类型**：扩展 `FactionId` enum（保持向后兼容 Attack/Defend/Neutral 三档）+ 新增 `NamedFactionId(u8)` 用于具名派系；或直接扩展 FactionId enum 加 variant——优先扩展 enum（与 serde 对齐）
- **跨仓库契约**：server 新增 `FactionStateV1` schema → `bong:faction_state` Redis 发布；agent 消费派系状态生成政治叙事（继承 narrative-political 路径）；client 无直接变更（派系信息通过 narration 呈现）
- **worldview 锚点**：§十一 散修江湖（派系定义）/ §九 交易生态（地盘控制资源）

---

## §0 三至五个具名势力初稿（待 worldview 核验后落稿）

> 具体名称/历史需对照 worldview + library 书籍后确认；以下为占位草案，P0 前必须收口

| 势力 | 地盘 zone | 风格 | 领袖境界 | 敌对方 |
|------|-----------|------|---------|--------|
| 青峰盟（QingFengMeng） | 青云残峰 zone | 散修中立守护者，保护灵草资源 | 固元上阶 | 血骨帮 |
| 血骨帮（XueguBang） | 死域边缘 zone | 骨币走私 + 抢劫专业户 | 凝脉巅峰 | 青峰盟 |
| 游商会（YouShangHui） | 跨区域游动 | 中立贸易者网络，操控骨币流通 | 通灵初阶（隐藏） | 无（两面调停） |

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | `FactionId` enum 扩展（3 具名派系 variant）+ `FactionRegistry` Resource（metadata 映射）+ schema `FactionStateV1` | ⬜ | 单测：FactionId serde 向后兼容（Attack/Defend/Neutral 不变）；FactionRegistry 返回正确 metadata；schema sample 双端校验 |
| **P1** | 领袖 NPC spawn（`NamedFactionLeader` Component + unique NpcTemplate）+ 地盘 zone 绑定（`FactionZoneClaim`）+ big-brain 领袖行为树（巡逻地盘 / 征收过路费） | ⬜ | 集成测试：领袖 NPC spawn 于正确 zone；FactionZoneClaim 与 FactionStore 一致；领袖 big-brain scorer 在地盘内激活 |
| **P2** | `bong:faction_state` Redis 发布（每 N 分钟快照）+ agent narration 消费（派系势力消长叙事） | ⬜ | 单测：Redis HASH 包含 faction_id / leader_alive / zone_control / member_count；agent mock 消费正确 FactionStateV1 反序列化 |
| **P3** | 玩家 × 具名派系信誉度接入（Renown 分派系独立维护）+ NPC 对话分支（高信誉 → 折扣 / 情报 / 私活） | ⬜ | 单测：不同 FactionId 的 Renown 独立存储不串台；NPC 对话 branch 按 Renown 阈值选择 |

---

## §8 开放问题（P0 决策门前需收口）

1. **FactionId 扩展方式**：直接加 enum variant 还是用 `NamedFactionId(u8)` 新类型？——建议直接扩展 enum 保持 serde 兼容；5 个以内 variant 不影响序列化性能
2. **地盘 zone 绑定颗粒度**：精确到 zone 名（`spirit_qi_zone_id`），还是绑定 world region 坐标块？——建议以现有 zone_name 字段为 key，避免引入新坐标系
3. **领袖 NPC 死亡后处理**：领袖死亡后派系是否崩溃还是选举新领袖？——v1 简化：领袖死亡 → 派系进入"无头"状态（`FactionStatus::Headless`），不自动选举；留 v2 处理
