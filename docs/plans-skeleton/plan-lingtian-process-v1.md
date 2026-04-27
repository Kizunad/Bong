# Bong · plan-lingtian-process-v1 · 骨架

**作物二级加工**（晾晒 / 碾粉 / 炮制 / 萃取）。在 plan-lingtian-v1 收获 ci_she_hao / ning_mai_cao / ling_mu_miao 等"原作物"基础上，加一层后处理产物作为 alchemy 丹方与日常消耗的中间形态。区分"鲜采直接投料"（损耗大、品质低）vs "炮制后投料"（损耗小、品质加成）。

**世界观锚点**：
- `worldview.md §十` 灵气零和——加工不无中生有灵气，只重新分配作物 quality 系数到产物
- `worldview.md §十二` 末法噬蚀——鲜采作物若不及时加工 / 收纳，72h 内灵气流失到 0（鼓励玩家加工锁定）
- `worldview.md §六` 真元只有染色谱——**禁止**"火炒火稻 → 火属性药粉"五行联动

**library 锚点**：`ecology-0002 末法药材十七种`（每种药材的传统炮制法描述）· 待写 `crafting-XXXX 末法炮制录`（各加工流程的世界观说明）

**交叉引用**：
- `plan-lingtian-v1.md`（herb item 输入端）
- `plan-alchemy-v1.md`（加工产物作为 pill_recipe 的高品质投料）
- `plan-forge-v1.md`（炮制器具 = forge 的子类工具：晾架/石臼/丹炉炮制模式/萃瓶）
- `plan-skill-v1.md`（herbalism / alchemy 双技艺，各加工类型有偏向）
- `plan-inventory-v1.md`（鲜采作物 vs 加工产物有不同保鲜度）

---

## §0 设计轴心

- [ ] 加工 = **作物 quality 转化器**：原作物 quality_accum [0.8, 1.5] → 加工产物 quality + duration_buff
- [ ] 4 类工艺（晾晒 / 碾粉 / 炮制 / 萃取）—— 每类一种器具、一种典型成品形态
- [ ] **保鲜度机制**：鲜采作物 72h 灵气线性流失到 0；加工产物保鲜 7 / 14 / 30 天不等
- [ ] 加工本身有失败率 / 损耗率，受 herbalism / alchemy 技艺等级影响
- [ ] 不引入"加工 → 加工 → 加工"链——最多 2 级（原 → 加工成品）

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·锁灵气**：鲜采作物的灵气尚未"定形"，会被天地噬散；加工 = 用工艺把灵气固定在产物结构里（晾晒去水分 / 萃取浓缩 / 炮制改性）
- **噬论·加工损耗**：加工过程同样被天地噬蚀——所以一定有 quality 损失或材料损耗
- **音论·器具偏向**：不同器具的"音"决定加工产物的属性（石臼 = 朴实色偏向 / 丹炉炮制 = 灼烈色偏向 / 萃瓶 = 凝实色偏向 / 晾架 = 自然色偏向）
- **影论·副本固化**：加工后的产物 = 原作物的"副本镜印"，不再是活物，所以可长期保存

---

## §2 4 类加工工艺

| 工艺 | 器具 | 输入 | 输出 | 时间 | 主导技艺 |
|---|---|---|---|---|---|
| **晾晒** | 晾架（户外，需阳光 tick） | 鲜采草本 ×N | 干品（保鲜 14d，quality ×0.9）| 1 in-game 日 | herbalism |
| **碾粉** | 石臼 | 干品 ×N | 药粉（投料效率 +20%，保鲜 7d）| 30 real-second/单位 | herbalism |
| **炮制** | 丹炉炮制模式 | 干品 / 鲜品 ×N + qi 5 | 炮制品（quality ×1.2，alchemy 加 +1 档成功率）| 5 min/批 | alchemy |
| **萃取** | 萃瓶（高阶） | 鲜品 ×3 | 萃液（量小但 quality ×2.0）| 10 min/批 | alchemy + herbalism |

---

## §3 数值梯度（按技艺等级）

```
herbalism Lv.0 ~ Lv.2：仅晾晒 / 碾粉，失败率 30% / 损耗 2 单位
herbalism Lv.3 ~ Lv.5：失败率 10% / 损耗 1 单位 + 解锁炮制初阶
herbalism Lv.6 ~ Lv.9：解锁萃取（需 alchemy Lv.3+）
alchemy Lv.3+：炮制成品 quality 加成 +0.1
alchemy Lv.6+：萃液产量 ×1.5
```

技艺与 plan-skill-v1 共用 progression。

---

## §4 保鲜度机制（plan-inventory-v1 / plan-cultivation-v1 接入）

- [ ] 鲜采作物在 inventory 内 tick `freshness: f32`（72h real-time 线性 1.0 → 0.0）
- [ ] freshness == 0 → item 转为"枯样"（quality ×0.3，仍可投料但效率极低）
- [ ] 进入加工 session 即冻结 freshness（加工完成后产物自带新 freshness 时长）
- [ ] **存入"灵气囊"**（plan-anqi-v1 灵物锁鲜）→ freshness 流失速率 ×0.3

---

## §5 数据契约

- [ ] `server/src/lingtian/processing.rs` —— `ProcessingKind` enum / `ProcessingSession` / `ProcessingRecipeRegistry`
- [ ] `assets/items/processed/` —— `dry_*.toml` / `powder_*.toml` / `processed_*.toml` / `extract_*.toml`
- [ ] `assets/recipes/processing.toml` —— 输入/输出/时间/技艺需求声明
- [ ] `server/src/inventory/freshness.rs` —— `FreshnessTracker` Component / `freshness_tick` system
- [ ] `server/src/forge/processing_mode.rs` —— forge 接入炮制 mode（如已有 forge 框架则补丁，否则等 plan-forge-v1）
- [ ] schema 扩展 —— `ProcessingSessionDataV1` payload，HUD 显示进度
- [ ] client `ProcessingActionScreen` —— 4 类工艺统合浮窗（参考 LingtianActionScreen 范式）

---

## §6 实施节点

- [ ] **P0**：晾晒 + 碾粉两类工艺 + ProcessingSession + 单测覆盖
- [ ] **P1**：FreshnessTracker + freshness_tick + 灵气囊接入 plan-anqi-v1
- [ ] **P2**：炮制 + 萃取（需 plan-forge-v1 至少 P0；否则用 placeholder block 实体）
- [ ] **P3**：客户端 ProcessingActionScreen + HUD 进度 + freshness UI tag
- [ ] **P4**：alchemy 接入 —— 加工产物作为 pill_recipe 的优选投料，给品质 / 成功率加成

---

## §7 开放问题

- [ ] freshness tick 是 wall-clock 还是 game-tick？离线时如何处理（玩家下线 7d 回来作物全枯 vs 暂停 tick）？
- [ ] 加工失败的产物去向——废料（接 plan-alchemy-recycle-v1 反哺 lingtian）vs 直接消失？
- [ ] 萃液保鲜短 / 量小 / quality 高，是否会成为"打 boss 前必嗑"meta 物？需要平衡吗？
- [ ] 4 类工艺的 UI 是统合一个 Screen 还是各 1 个浮窗（保真原 plan 风格）？
- [ ] 作物"枯样"是否还能拿来反哺 lingtian（plan-alchemy-recycle 衔接）？

---

## §8 进度日志

- 2026-04-27：骨架创建。前置 `plan-lingtian-v1` ✅；`plan-alchemy-v1` ⏳（已有 P0 框架）；`plan-forge-v1` 状态待核（炮制依赖之）。建议本 plan P0–P1 不依赖 forge，先用纯 ECS session 起步。
