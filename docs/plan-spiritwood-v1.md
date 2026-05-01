# Bong · plan-spiritwood-v1 · 骨架

**灵木材料体系**。灵木（`ling_mu`）是末法残土最顶级的真元载体之一，与异兽骨骼并列——"飞 50 格保留 80% 真元"。server 侧已有 `MegaTreeKind::SpiritWood`（spawn 区唯一地标级巨树），但其木料至今没有对应 item、没有 forge/weapon 集成，`ling_wood` 在 plan-forge-v1 中仍是占位名。本 plan 补全灵木 item 体系、采集流程、forge/weapon 载体用途、封灵匣制作，正典化占位名为 `ling_mu`。

**世界观锚点**：`worldview.md §四 战斗系统 / 暗器流载体`（"异变兽骨/灵木（优良）：飞 50 格保留 80% 真元"）· `worldview.md §十 资源与匮乏 / 资源表`（"载体材料：异兽骨骼、灵木 | 高 | 器修/暗器流核心耗材"）· `worldview.md §十六.三 封灵匣/负灵袋`（灵木 + 异兽骨骼编制保养容器，炼器流专属饭碗）

> **注**：本 plan §0/§3 "灵木砍倒即永久消失" 是 plan 自定的稀缺约束——worldview 当前**无此明文**（§七 是动态生物生态非末法约束章；§十 仅标"稀缺度高"）。建议升 active 后通过独立 PR 在 worldview §十 资源与匮乏末尾补"稀缺资源永久消耗"小节正典化（**严禁本 plan 自动改 worldview**）。

**library 锚点**：`docs/library/ecology/ecology-0002 末法药材十七种.json`（夜枯藤章节提到灵气吸附植物体系，灵木属高阶节点）· 待写 `crafting-XXXX 灵木解析`（灵木采集禁忌 / 锯解方法 / 品阶鉴别，anchor worldview §四 + §十）

**交叉引用**：
- `plan-worldgen-v3.1`（`MegaTreeKind::SpiritWood` 已实装，唯一坐标在 spawn 区；本 plan 在其伐木事件挂 drop 钩子）
- `plan-botany-v1`（✅；采集 session 模式复用 `BotanyHarvestSession`）
- `plan-forge-v1`（✅；`ling_wood` placeholder → `ling_mu` 正典化）
- `plan-weapon-v1`（暗器/飞剑用灵木为载体，提升真元保持率）
- `plan-fauna-v1`（封灵匣需要 `ling_mu` + `feng_he_gu` 异兽骨骼联合制作）
- `plan-zhenfa-v1`（灵木作为阵法高阶载体；plan-zhenfa §3 表中已列）
- `plan-mineral-v2`（P1 采矿 session 复用思路 → 灵木采集 session 同款）

**阶段总览**：
- P0 ⬜ item 定义（灵木原木 / 灵木板材 / 灵木精粹）+ 采集 drop 链路
- P1 ⬜ 采集 session（伐木耗时 + 取消条件 + 灵木消耗 / 再生规则）
- P2 ⬜ forge 正典化（`ling_wood` → `ling_mu`，batch replace）
- P3 ⬜ 封灵匣 recipe（`ling_mu` + `feng_he_gu` → `ling_xia`，保养容器）
- P4 ⬜ 阵法载体接入（plan-zhenfa §3 表中"灵气浸润方块"扩充为 `ling_mu` 特殊属性）

---

## §0 设计轴心

- [ ] **稀缺性是核心设计压力**：spawn 区仅一棵 SpiritWood 巨树（`seed_spacing: 2000`）；砍倒即永久消失，无快速再生——玩家必须在"使用"和"保护世界地标"间权衡
- [ ] **品阶**：灵木原木（低级）→ 灵木板材（中级，锯解加工）→ 灵木精粹（高级，结晶化，需炼器工序）三级，越高越难得
- [ ] **不做"伐木刷新"**：灵木不像普通树木一样重新生长；唯一来源是砍 SpiritWood 巨树（每棵产量有限）
- [ ] **可选的"只取部分"机制**（v2+）：只砍若干枝干，不整棵伐倒 → 减少对地标的破坏，产量也更少

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·灵木封印**：灵木在漫长生长中吸收并"缚"住了大量灵气——其纹理本身就是一条条微型封印线路，这是它作为顶级真元载体的物理根据
- **影论·次级投影稳定**：灵木致密纹理让投射物上的真元"镜印"更稳定（次级投影衰减更慢）——与异兽骨骼机理类似但来源不同
- **噬论·砍伐后急速衰退**：脱离树根后，灵木的真元缓慢泄漏（freshness 机制），成材后若不及时加工则降品阶
- **音论·灵木共鸣**：灵木板材接触真元时会发出细微共鸣音（高灵敏玩家可感知），这是鉴别真假灵木的关键——防止以次充好

---

## §2 item 分级

| item_id | 获取方式 | 品阶 | 主要用途 |
|---|---|---|---|
| `ling_mu_gun`（灵木原木）| 砍 SpiritWood 巨树 → drop | 中品灵 | 阵法载体（12 小时镜印）/ 基础 forge 原料 |
| `ling_mu_ban`（灵木板材）| `ling_mu_gun` × 3 → 木工加工 session | 上品灵 | 暗器/飞剑主体 / 封灵匣材料 |
| `ling_mu_jing`（灵木精粹）| `ling_mu_ban` × 5 + 真元 20 → 炼器炉 | 极品灵 | 顶级武器载体 / 封灵匣高阶版 |
| `ling_xia`（封灵匣）| `ling_mu_ban` × 2 + `feng_he_gu` × 2 → 炼器 | 成品 | 法宝保养容器（使用才扣耐久）|

---

## §3 采集 session（灵木伐木）

> 复用 plan-botany-v1 `BotanyHarvestSession` 模式；plan-mineral-v2 P1 采矿 session 同款。

```
触发：
  玩家手持斧类工具 右键 SpiritWood 巨树树干方块
  → 检查 工具品阶 ≥ pickaxe_tier_min_equivalent（凡铁斧不够，需灵铁斧）
  → 启动 WoodSession { player, log_pos, ticks_total }

进度：
  ticks_total = 240t（~12s，远长于普通树）
  进度条 bong:lumber_progress 下发 HUD

取消条件：
  玩家移动 > 1.5 格 / 切换物品 / 受伤 → 中断，无 drop

完成：
  → BlockState 方块变 AIR（消除该段树干）
  → spawn ItemEntity: ling_mu_gun × (2~4, RNG)
  → freshness 挂载（profile: ling_mu_gun_v1，24h 半衰期，脱树后开始计时）

再生：
  无再生。SpiritWood 巨树树干方块被全部砍完后永久消失。
  "末法残土没有再生。"
```

---

## §4 forge 正典化

> plan-forge-v1（✅）及 plan-weapon-v1 中使用占位名 `ling_wood`，需批量替换。

- [ ] **server 替换**：`server/src/forge/steps.rs:412` / `mod.rs:617` 等 `"ling_wood"` → `"ling_mu_ban"`（板材为 forge 主用）
- [ ] **assets 替换**：`server/assets/forge/blueprints/*.json` 中 `material_id = "ling_wood"` → `"ling_mu_ban"`
- [ ] **lingtian 灵木苗**（`server/src/lingtian/systems.rs:2616` 的 `display_name: "灵木苗"`）：这是灵田种植的幼苗，item_id 改为 `ling_mu_miao`，与 `ling_mu_gun` 区分（幼苗 ≠ 成材原木）
- [ ] **tests**：forge blueprint 加载命中 `ling_mu_ban`；伐木 drop 测 `ling_mu_gun`；lingtian 灵木苗种植命中 `ling_mu_miao`

---

## §5 封灵匣制作

> worldview §十："器修/炼器师用异兽骨骼、灵木编制的保养容器——上古遗物放在匣中时不因佩戴、运输、磕碰而磨损消耗轮数，只有真正使用才扣次数。"

- [ ] **Recipe**：`server/assets/forge/blueprints/ling_xia_v1.json`（炼器炉 + `ling_mu_ban` × 2 + `feng_he_gu` × 2 → `ling_xia` × 1）
- [ ] **`ling_xia` 功能**：道具类 item，放入背包后"保养槽"接受 1 件法宝——该法宝在 `ling_xia` 内时磨损暂停（`shelflife / durability_tick` 不计）；取出即恢复计时
- [ ] **背包 UI 扩展**（P3+）：需要 plan-inventory-v1 补充"封灵匣格"拖拽目标 → 归 inventory plan 实装
- [ ] **tests**：法宝放入 ling_xia → durability_tick 暂停；取出 → 恢复；ling_xia 本身不占法宝格（是容器，不是武器）

---

## §6 阵法载体接入

> plan-zhenfa §3 载体表已列"灵气浸润方块（馈赠区原产）| 2 小时"；灵木板材可比肩甚至超越。

- [ ] **镜印保持时长**：`ling_mu_ban` 作阵法载体 → 镜印保持 12 小时（介于夜枯藤 12h 和异变兽核镶嵌方块 24h 之间）
- [ ] **`ling_mu_gun`**（原木）作载体 → 4 小时（未加工，稍低于板材）
- [ ] 接入 plan-zhenfa 的 `CarrierMaterial` 注册表（待 zhenfa P0 定义后对接）

---

## §7 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|------|
| `WoodSession { player, log_pos, ticks_total }` component | `server/src/spiritwood/session.rs`（新文件）|
| item toml × 4 | `server/assets/items/spiritwood/` |
| `ling_mu_ban` 全量替换（ling_wood）| `server/src/forge/*.rs` + `server/assets/forge/blueprints/*.json` |
| `ling_mu_miao` item（灵木苗正典化）| `server/assets/items/spiritwood/ling_mu_miao.toml` |
| `ling_xia_v1.json` blueprint | `server/assets/forge/blueprints/` |
| shelflife profile `ling_mu_gun_v1` | `server/src/shelflife/registry.rs` |
| `bong:lumber_progress` channel | `server/src/schema/channels.rs` |

---

## §8 实施节点

- [ ] **P0**：4 种 item toml + `WoodSession`（伐木 session，复用 BotanyHarvestSession 框架）+ SpiritWood 方块 drop 钩子 + freshness 挂载 + 单测（session 完成 drop / 中断无 drop / 非灵木树干拒绝）
- [ ] **P1**：工具品阶门槛 + `bong:lumber_progress` 进度条 + freshness profile 注册 + 24h 衰减 e2e 测
- [ ] **P2**：forge 正典化（`ling_wood` → `ling_mu_ban` batch replace + 全绿）+ lingtian 灵木苗 id 修正
- [ ] **P3**：封灵匣 recipe + `ling_xia` 功能（法宝保养暂停耐久）+ forge e2e
- [ ] **P4**：阵法载体 12h 接入（等 plan-zhenfa P0 `CarrierMaterial` 定义后对接）

---

## §9 开放问题

- [ ] SpiritWood 是否只有 spawn 区一棵（当前 `seed_spacing: 2000` 意味着 2000 格内唯一）？全服砍完后再无来源，是设计意图还是需要多生成几个远距离点？
- [ ] "只取部分"机制（v2+）：保留树冠结构 + 只砍若干树干节点，产量 30%，树貌保存 → 需 worldgen 方块级追踪，较复杂
- [ ] `ling_mu_jing`（灵木精粹）炼器工序：是走 plan-forge-v1 炼器炉还是专用"灵木精炼 session"？
- [ ] 封灵匣上限：一个匣只保 1 件法宝？还是 3 件（小匣/中匣/大匣）？
- [ ] 灵木板材 vs 夜枯藤（阵法载体竞品）经济关系：板材 12h vs 夜枯藤 12h 同价，靠稀缺性自然分层 or 需要 buff 板材使其有差异化？
- [ ] 林中小灵木苗（lingtian 灵木苗）能否在灵田里长成可采伐的 SpiritWood 幼树（超长周期，v3+ 考虑）？

---

## §10 进度日志

- **2026-04-27**：骨架立项。来源：`docs/plans-skeleton/reminder.md` "通用/跨 plan"节（`plan-spiritwood-v1 待立`）+ plan-forge-v1 `ling_wood` 正典化缺口。server 侧 `MegaTreeKind::SpiritWood` 巨树已在 worldgen 实装（`server/src/world/terrain/mega_tree.rs`，spawn 区，trunk_height 140–180 格）；`server/src/lingtian/systems.rs:2616` 有 `"灵木苗"` 展示名但无 item_id。`server/src/forge/` 中 `"ling_wood"` 占位 2 处待替换。
