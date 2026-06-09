# plan-furniture-buff-v1 — 家具/光源放置与安居恢复 buff(骨架)

> 一句话:放置类僵尸物品「家具光源建造」消杀——纯方块 4 个(火把/灯笼/门闩/窗栅)直接落地放置,家具 4 个(床/蒲团/灵石架/防潮架)放置后给恢复速度 +x% 类 buff 并加限制。
>
> 来源:放置类 17 调查 workflow 2026-06-10(opus 抽查 7/7 证据属实);用户拍板:家具给体力/血量恢复 +x% 并限制。

**依赖**:plan-block-lifecycle-v1 P4(client 放置 wiring)合入 main 后开 P0。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 纯方块 4 个落地(category 改写 + vanilla state 映射) | ⬜ |
| P1 | FurnitureRegistry + 家具方块放置(bong_blocks 扩) | ⬜ |
| P2 | 血量自然恢复 tick(从零建)+ HealthRegenBoost | ⬜ |
| P3 | 床/蒲团 aura buff + 限制 | ⬜ |
| P4 | 防潮架接保鲜 / 灵石架降级装饰 / 视听 | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - 放置管线:`world/block_place.rs:212-226`(category 双闸)+ `:243-253`(`block_item_to_state` 仅 6 条 vanilla 映射)+ `bong_blocks.rs`(`place_bong_block` 拒 vanilla state,有 pin 测试)
  - client wiring:`ClientRequestSender.java:156` `sendBlockPlace`(P4 合入前是死代码,**本 plan 不重复实现**)
  - 恢复系统:`combat/lifecycle.rs:207` `stamina_tick` + `StaminaRecovBoost`(status.rs:264 乘区先例);血量**无自然恢复 tick**(只有 `wound_bleed_tick` lifecycle.rs:158)
  - 打坐:`practice_session.rs:68` 两核心函数 `#[allow(dead_code)]` 未注册 ECS——蒲团接它=先接活
- **出料**:
  - `FurnitureRegistry`(仿 `ZhenfaRegistry` HashMap<[i32;3],Entity>,zhenfa/mod.rs:309)
  - 新 `health_regen_tick` 基准恢复系统 + `StatusEffectKind::HealthRegenBoost`(events.rs:81 新变体)
  - buff 施加一律 `ApplyStatusEffectIntent`(events.rs:159),**禁止直写 DerivedAttrs / recover_per_sec**(`sync_stamina_regen_from_realm` movement/mod.rs:183 每帧硬写会覆盖直写值)
  - 顺带激活死字段:`healing_rate_multiplier`(components.rs:291,scar_circuit 写入无读取)接入 health_regen_tick 乘区
- **共享类型 / event**:复用 StatusEffectKind 乘区体系;不新造恢复通道
- **跨仓库契约**:server `BlockPlaceRequest`(lifecycle 已落);client 放置走 lifecycle P4 的 interactBlock 分支;agent 不参与
- **worldview 锚点**:§十四「这个世界的一天」(作息恢复);§十一 安全与社交(门闩/窗栅防贼);凡物买卖锚定 §九 骨币
- **qi_physics 锚点**:无。体力/血量是物理量;蒲团若给 `QiRegenBoost` 走已有 status 变体(其底层 regen tick 自有 zone drain 守恒,tick.rs:88),本 plan 不新增真元通道。

---

## P0 — 纯方块 4 个落地

- `torch_item`/`lantern_item`/`door_bolt`/`window_grate` category `misc`→`block`(workbench_materials.toml)
- `block_item_to_state`(block_place.rs:243)加 4 条映射:torch→`BlockState::TORCH`(光自动传播,blocks.rs:63)、lantern→`LANTERN`(光15)、door_bolt/window_grate→实体 vanilla 占位(铁门/铁栏杆类)
- 测试:4 个物品放置 e2e(give→放置→world 出方块→破坏掉落);category 改写后 grid/重量回归

## P1 — FurnitureRegistry + 家具方块放置

- `simple_bed`/`meditation_mat`/`moisture_base`/`spirit_stone_rack` 走 bong_blocks.json 新方块定义 + codegen + `block_item_to_state` bong arm
- 新 `FurnitureRegistry` 登记放置坐标→entity;破坏时移除
- 测试:registry 增删一致性;重启持久化(对齐 lifecycle 的方块持久化机制)

## P2 — 血量自然恢复 tick(从零)+ HealthRegenBoost

- 新 `health_regen_tick`:基准慢速恢复(数值见 §8 #1),读 `healing_rate_multiplier` 乘区(激活死字段,SpleenKidney 经脉加成从静默失效变生效)
- `StatusEffectKind::HealthRegenBoost` 新变体 + status.rs 乘区分支 + wire 映射(proto_convert + combat_bridge)
- 测试:基准恢复速率/流血时不恢复(优先级)/乘区叠加 clamp/变体 pin 测试

## P3 — 床/蒲团 aura buff + 限制

- furniture aura tick:`FurnitureRegistry` 范围扫描(Chebyshev,参 `is_within_workbench_range` workbench.rs:47),范围内玩家 `send(ApplyStatusEffectIntent)`
  - simple_bed → `HealthRegenBoost`(+x%,数值 §8 #1)
  - meditation_mat → `QiRegenBoost` / `CultivationAcceleration`(已有变体,events.rs:81)+ 接活 `practice_session_tick`
- **限制**(用户硬要求):① 同效 buff 不叠加(aura tick 按 target 已有 status 去重)② 辐射半径小(单间屋量级)③ 仅静止/坐卧姿态生效(移动即掉 buff)④ 凡物不设数量上限但效果不叠 → 多放无收益
- 视听:上 buff 瞬间一次性粒子(床=暖黄羽絮 BongSpriteParticle burst 6 颗 lifetime 12t #E8C97A;蒲团=青白雾圈 radial 8 颗 #BFD8C8)+ HUD 事件流一条「歇息中,恢复加快」;无常驻 UI(HUD 极简原则)
- 测试:范围内/外边界 off-by-one;移动掉 buff;双床去重;buff 过期回基准

## P4 — 防潮架接保鲜 / 灵石架降级 / 收尾

- `moisture_base`:接 shelflife decay modifier(compute.rs:37 占位)——架上(范围内容器)湿度衰减归零
- `spirit_stone_rack`:灵石存储/磨损系统不存在 → **降级纯装饰方块**(description 改),复活留给将来灵石系统 plan
- `niche_repair_kit` 不在本 plan(见 [[plan-niche-craft-fix-v1]])
- 测试:moisture_base 范围内 shelflife 曲线变化对照;装饰方块放置/破坏回归

---

## §8 开放问题(P0 决策门前需收口)

1. **数值**:health_regen 基准速率(建议 0.5HP/10s 量级,流血压制)、床 +x%(建议 +50%)、蒲团 qi regen +x%(建议 +20%,凡物弱效)
2. **床的使用形态**:纯 aura(站旁边就行)vs 必须"躺"(交互姿态,工作量大)——建议 P3 先 aura,姿态留 v2
3. **moisture_base 的"架上"判定**:范围内所有容器 vs 直接放其上的容器(需位置关系判定)
4. **门闩/窗栅的"防贼"**:本 plan 只做实体方块阻挡,NPC 破门/盗窃系统不存在,效果=物理屏障即可?
5. **与 plan-block-lifecycle-v1 P5 的边界**:lifecycle P5 若已含部分 category 改写,P0 按其落地范围收缩(实施前 grep 确认)
