# Bong · plan-tools-v1 · 骨架

**采集 / 加工工具体系**。末法残土修士使用凡器（无真元加持的低品阶工具）执行采集 / 制作 / 加工动作；不持工具空手采特殊植物 / 矿物 / 兽骨 → 真元烫伤、霜伤、负压割伤、骨刃割伤等真伤口。本 plan 定义 7 件初版工具的 item 体系、工具识别 component、采集/制作 session 接入点，承接 plan-botany-v2 / plan-mineral-v2 / plan-fauna-v1 / plan-forge-v1 的"required_tool"声明。

**世界观锚点**：
- `worldview.md §四 战斗系统`（距离衰减——凡器无真元投射，仅近身使用，采集动作天然贴身）
- `worldview.md §四 战斗系统`（战力分层 / 拼刺刀——凡器是底层修士的常态装备，与"暗器流真元载体"区分；工具不参与战斗 stats）
- `worldview.md §五 战斗流派`（暗器流"异变兽骨/灵木 = 优良真元载体"对照——凡器命名禁用"灵\*"词头，避免越级到法宝层级；优选"采药刀 / 刨锄 / 骨骸钳"等凡俗手作名）
- `worldview.md §十 资源与匮乏`（工具本身要消耗矿物 / 兽骨 / 灵木制作——与 plan-mineral-v2 / plan-fauna-v1 / plan-spiritwood-v1 的资源闭环挂钩）

> **注**：worldview 当前无"凡器"独立章节（§三 = 修炼体系，无"末法命名原则"小节）。本 plan 把"凡器"概念当作 §四/§十 隐性外延落地；后续若需正典化，可考虑在 worldview 加 "§四.X 凡器与凡夫战力"小节，本 plan 暂不强求。

**library 锚点**：待写 `peoples-XXXX 残土凡器谱.json`（七件工具的工艺、材料、传承故事；需 anchor worldview §三 命名 + §五 资源稀缺）

**交叉引用**：
- `plan-botany-v2.md`（active；P0–P3 退化 `required_tool=None` → `dispersal_chance=1.0`，本 plan 落地后 P4 回填 WoundOnBareHand 真伤——见 plan-botany-v2 §"风险与缓解"）
- `plan-fauna-v1.md`（骨架；屠宰刀 / 骨骸钳从异兽尸体取骨需特定工具——本 plan 与 fauna-v1 协同定义"屠宰会话"）
- `plan-mineral-v2.md`（骨架；刨锄 / 凿子从矿脉采矿——v1 当前直接 `BlockBreakEvent` 给 drop，无工具需求；v2 是否引入工具门槛由 mineral-v2 自决）
- `plan-forge-v1.md`（已归档；锻造系统会消费工具但不直接定义工具——本 plan 的 7 件工具走 forge blueprint 流程作为"产品"，与 forge 数据模型对齐）
- `plan-zhenfa-v1.md`（active；阵法布置不需工具——但布置笔可作为后续 v2 扩展）
- `plan-shelflife-v1.md`（active；工具是否有耐久度 / 损耗——见 §"开放问题"）

**阶段总览**：
- P0 ⬜ 7 件工具 item toml + `ToolKind` enum + `ToolTag` component + 主手识别 helper（采集/制作 session 调用入口）
- P1 ⬜ 工具 forge blueprint 接入（每件工具一份 blueprint：材料 + 步骤 + 难度）
- P2 ⬜ botany-v2 WoundOnBareHand 真伤回填：玩家空手 / 错工具采 v2 高阶物种 → 触发对应伤口档（依 plan-botany-v2 P4 节奏）
- P3 ⬜ fauna 屠宰会话：骨骸钳 / 屠宰刀从异兽尸体取骨（drop 链可选分支）
- P4 ⬜ 工具耐久度 / 损耗（接 plan-shelflife-v1，可选；如最终决定不引耐久则 P4 删）

---

## §0 设计轴心

- [ ] **凡器只是工具**：不参与战斗 stats、不挂真元、不参与暗器投射；锋利度 / 材质决定能否采指定物种，不决定攻击伤害——避免与 plan-weapon 体系混淆
- [ ] **空手不是默认采集动作**：worldview §四"末法散修无装备就只能空手刨"——空手能采的只是"低品阶不挂 hazard 的草药 / 浆果"；高阶物种 / 兽骨 / 矿脉空手必有代价（伤口、产量减半、品阶下降）
- [ ] **工具 ↔ 物种锚定**：每个 v2 物种 / 异兽 / 高阶矿在 plan-botany / fauna / mineral 内自报 `required_tool: Option<ToolKind>`——本 plan 不维护这张表，只暴露 `ToolKind` enum 和"是否持有"的查询 API
- [ ] **不引入 NBT 复杂状态**：工具就是物品（item_id），不挂 freshness / qi / decay 等 NBT；除非 P4 明确引入耐久度
- [ ] **末法命名锚定**：禁用 "玄铁刀 / 灵犀镰" 等上古词头；**同时避用"灵\*"作工具修饰前缀**——凡器无封真元能力（见 §1 缚论），挂"灵"字头会与"灵宝/法器"层级混淆。优选 **采药刀 / 刨锄 / 草镰 / 冰甲手套 / 骨骸钳 / 钝气夹 / 刮刀** 等贴近凡俗手作语境的命名

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·凡器无封真元**：末法天地灵气稀薄，普通铁器 / 木柄无法长期封存真元——所以工具只是物理介质，不能像暗器载体那样投射真元
- **噬论·空手代价**：植物 / 兽骨结构本身吸真元（散气物种）/ 含霜晶（雪魄莲）/ 带骨刃（异变兽脊骨）—— 空手接触必有真伤回馈；工具是隔离层
- **音论·工具无共振**：凡器不参与符术 / 阵法的振动谐振——工具只在 session 期间起到"机械隔离 + 增产"的物理作用
- **影论·工具不投影**：投射、镜印、暗器流不属于工具系统——本 plan 严格限定在"贴身/近距离手作"

---

## §2 七件工具 item 列表（草稿）

| item_id | 朴拙名 | 主要用途 | 制作材料（待 forge blueprint） | 锚定物种 / 锚定 hazard |
|---|---|---|---|---|
| `cai_yao_dao` | 采药刀 | botany 通用低-中阶物种采集 | 凡铁 + 木柄 | 通用——避开"散气" hazard |
| `bao_chu` | 刨锄 | botany 根系 / 块茎 + mineral 软土矿 | 凡铁 + 木柄 | 根系类植物（如 yuan_ni_hong_yu） |
| `cao_lian` | 草镰 | botany 草本 / 蔓藤类 | 凡铁刃 + 木柄 | 蔓藤类（如 bei_wen_zhi） |
| `bing_jia_shou_tao` | 冰甲手套 | botany 低温物种隔离体温 | 冰原狼皮 + 导冷金属片†（fauna + mineral 联产）| `xue_po_lian`（雪魄莲，体温即化）|
| `gu_hai_qian` | 骨骸钳 | fauna 取异变兽骨 + botany 骨刃类 | 异兽骨 + 凡铁绞架 | `lie_yuan_tai`（裂元胎，骨刃护身） |
| `dun_qi_jia` | 钝气夹 | botany 高真元密度物种钝化真元导通 | 钝气合金† + 凡铁柄 | `fu_yuan_jue`（负元决，近身吸真元） |
| `gua_dao` | 刮刀 | botany 树皮 / 树脂类 + 矿石表面剥离 | 凡铁 + 木柄 | 树皮类（如 xuan_gen_wei） |

> †**材料命名 TODO**：表中"导冷金属片 / 钝气合金"为占位描述名，正典化材料名（金属合金 / 木柄品阶等）由 `plan-mineral-v2.md` / `plan-spiritwood-v1.md` 决定后回填。本骨架避免擅自定义"灵铁 / 灵木"等带"灵"字头的材料名。

> 7 件全部为"凡器"——制作不消耗骨币，材料门槛低；plan-forge-v1 已实装的 `furnace_fantie` 凡铁炉即可全部产出（仅 `bing_jia_shou_tao` / `gu_hai_qian` 需 plan-fauna-v1 立后才能凑材料）。

---

## §3 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|------|
| `ToolKind` enum（7 变体） | `server/src/tools/kinds.rs`（新文件） |
| `ToolTag { kind: ToolKind }` component | `server/src/tools/components.rs`（新文件） |
| `fn player_holding_tool(player: &Player, kind: ToolKind) -> bool` | `server/src/tools/query.rs`（新文件） |
| `fn item_kind_to_tool(item_id: &str) -> Option<ToolKind>` | `server/src/tools/registry.rs` |
| 7 件 item toml | `server/assets/items/tools/` |
| forge blueprint × 7 | `server/assets/forge/blueprints/tools/` |
| `WoundOnBareHand` 真伤分发（被 plan-botany-v2 调用）| `server/src/botany/harvest_hazard.rs`（**plan-botany-v2** P4，本 plan 仅暴露"是否持工具"查询） |

---

## §4 实施节点

- [ ] **P0**：`ToolKind` enum + `ToolTag` + 7 个 item toml + `player_holding_tool` 查询 helper + 单测（每 ToolKind 各一条 happy + 错工具 + 空手三档）
- [ ] **P1**：7 份 forge blueprint（套 plan-forge-v1 既有 step 模型）+ 锻造产出 e2e 单测（每件工具能从材料锻出来）
- [ ] **P2**：botany-v2 接入——`HarvestHazard::WoundOnBareHand { required_tool, wound_tier }` 真分发（玩家持 required_tool 则跳过，否则调 plan-injury 系统挂对应 wound_tier）；e2e：玩家持采药刀 vs 空手采 fu_yuan_jue → 后者真挂"主手真元烫伤"
- [ ] **P3**：fauna 屠宰会话——`ButcherSession`（fauna NPC death 后 2 分钟窗口，玩家持骨骸钳点击尸体启动 session → 抽 drop table 取骨）；可选分支，不影响 P0–P2
- [ ] **P4**（可选）：耐久度——若引入，每个工具挂 `Durability(u16)`，每次 session -1；归零 → 物品消失；接 plan-shelflife-v1 的"线性衰减"档作 fixture 范例

---

## §5 验收

| 阶段 | 验收条件 |
|---|---|
| P0 | 7 件 item 可生成 / 拾取；`player_holding_tool` 在测试场景中返回正确；§3 单测全绿 |
| P1 | 凡铁炉单测中能从原料 → 7 件工具各自的成品；blueprint JSON 加载单测覆盖 |
| P2 | 跨 plan e2e：botany-v2 fu_yuan_jue 采集 session，无 dun_qi_jia → 真挂 wound；持夹则不挂 |
| P3 | fauna NPC 死亡 → 持骨骸钳 → ButcherSession 启动 → drop table 抽取异兽骨 |
| P4（如启用）| 工具耐久 N 次后消失；shelflife profile 链路单测覆盖 |

---

## §6 风险

| 风险 | 缓解 |
|---|---|
| 与 plan-weapon-v1 的"武器"概念混淆 | 工具 component (`ToolTag`) 与武器 component (`WeaponStats`) 完全不同 module；同一 item 不允许同时挂两者；编译期/runtime 双重防御 |
| 7 件工具可能不够 / 太多 | 起步 7 件覆盖 botany-v2 全部 hazard 类型；后续按 plan-mineral-v2 / fauna-v1 按需扩 |
| 冰甲手套 / 骨骸钳依赖 fauna 材料（未立） | P0 仅做 enum + 占位 toml；P1 forge blueprint 等 fauna-v1 P0 落地后再拼材料；plan 流转不卡 |
| 材料名（导冷金属片 / 钝气合金 / 凡铁绞架）命名漂移 | 本骨架仅占位描述；正典化由 mineral-v2 / spiritwood-v1 / forge-v1 决定，落地前 grep 对齐——避免与"灵铁 / 灵木"等老套词混用 |
| 耐久度引入后玩家体感繁琐 | P4 设为可选；若决定不做则**删掉 P4**，避免半实装 |

---

## §7 开放问题

- [ ] **耐久度**：是否引入？（worldview 未明确锚定；引入则增加 inventory 复杂度，不引入则工具是"一次买永久用"——倾向**不引入**，让玩家压力来自材料采集而非工具维护）
- [ ] **工具升级 / 品阶**：凡铁刀 → 灵铁刀 → 极品刀 ？（本 plan 不引入；如未来需要，新开 plan-tools-tier-v2）
- [ ] **多人协作工具**：双人锯 / 协作炉等（非 P0–P4 范畴）
- [ ] **与 plan-shelflife-v1 的 freshness**：工具会不会受潮 / 锈蚀？（不引入——增加管理成本，玩家收益感弱）
- [ ] **采集动作的 input 模型**：右键长按蓄力 / 直接 use item / 进入 HarvestSession UI ？（与 plan-botany-v2 / plan-HUD-v1 协同决议；本 plan 不强制）
- [ ] **凡铁 / 灵铁**：plan-mineral-v2 的命名是否已正典化（`fan_tie` / `ling_tie`）？（依 plan-mineral-v2 立项时序；本 plan blueprint 引用前需校对）

---

## §8 进度日志

- **2026-04-29**：骨架立项。来源：plan-botany-v2 §"风险与缓解" 列出"WoundOnBareHand 在工具系统未实装时变成 100% 空手 wound"——P0–P3 退化为 `dispersal_chance=1.0`，待本 plan P2 回填真伤。7 件工具列表已由 plan-botany-v2 §394 / §403 点名。当前 server/agent/client 无 `tools` 模块，无 `ToolKind` enum，无相关 item toml。
