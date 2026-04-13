# 战斗系统客户端 UI 实现计划 V1

> 从 `plan-combat-v1.md` 拆出的**客户端 UI 分支**。本文档描述 Fabric 客户端需要渲染/交互的所有战斗相关界面。
>
> **拆分理由**：UI 开发需要实时视觉反馈，不适合云端 LLM 并行。server/schema 部分留在 `plan-combat-no_ui.md` 由云端推进，UI 在本地（Claude + 手动测试）迭代。
>
> **前置依赖**：
> - `plan-combat-no_ui.md` 定义的 IPC schema 已定稿（`bong:combat/combat_event` / `status_snapshot` / `derived_attrs_sync` / `death_screen` / `terminate_screen`）
> - `plan-HUD-v1.md` 的两层快捷栏、三状态条基础已落地
> - `plan-inventory-v1.md` 的 inspect 屏（伤口层骨架、经脉层）已合并
> - 客户端 owo-lib UI 层（OwoUIDrawContext、CustomPayload 路由）已可用

---

## 0. 设计约束

1. **所有 UI 数据由 server 推送**，client 不自算（保持 server 权威）
2. **读取沿用 `ServerDataRouter` 统一 dispatch**（见 `bong:server_data` 协议）；C2S 行为通过 `bong:client_request` 回写
3. **沉浸式极简**（参见 user memory/HUD 反馈）：常驻仅两层快捷栏 + 三状态条 + 事件流；战斗特有界面**按需弹出**，不常驻
4. **特效/动画优先低成本方案**：Minecraft 粒子 / 文字飘字 / 颜色闪烁 > 自研 shader

---

## 1. UI 总览（从 `plan-combat-v1.md §12` 迁入）

| UI 组件 | 数据来源 | 常驻? | 草图 |
|---|---|---|---|
| **inspect 伤口层** | `Wounds[]` 实时同步（inspect 屏已有骨架） | inspect 内 | [svg](./svg/inspect-wounds.svg) |
| **inspect 状态面板** | `StatusEffects` 全量（按 kind 分组） | inspect 内 | [svg](./svg/inspect-status.svg) |
| **inspect 武器/法宝检视** | `Weapon` tooltip / `Treasure` 展开 | inspect 内 | [svg](./svg/weapon-treasure.svg) |
| **真元条（战斗扩展）** | `throughput_current` 峰值高亮覆盖真元条 | 常驻 | [svg](./svg/hud-combat.svg) |
| **Stamina 条** | 独立于 qi/health，跑/冲刺掉 | 常驻 | [svg](./svg/hud-combat.svg) |
| **HUD 顶部状态效果栏** | 最多 8 格（DoT 优先→控制→加成），图标 `source_color` 描边 | 按需（有效果时） | [svg](./svg/hud-combat.svg) |
| **DerivedAttrs 大图标** | 飞行/虚化/渡劫锁定等特殊状态 | 按需（有状态时） | [svg](./svg/hud-combat.svg) |
| **法术体积滑块面板** | radius / velocity_cap 双滑块，实时预览 | 施法时弹出 | [svg](./svg/attack-panels.svg) |
| **施法 qi_invest 滑块** | "N 格内可命中" 提示 | 施法时弹出 | [svg](./svg/attack-panels.svg) |
| **防御 UI** | 截脉 200ms 弹反提示 / 涡流键 / 伪皮层 | 按需 | [svg](./svg/defense-ui.svg) |
| **暗器制作面板** | ForgeWeaponCarrier（选物+注真元+计时） | 主动打开 | [svg](./svg/attack-panels.svg) |
| **阵法布置 UI** | 方块选择+触发类型+注真元 | 主动打开 | [svg](./svg/attack-panels.svg) |
| **死亡画面** | 运数 + 遗念 + 重生/终结 + 60s 倒计时 | 致死时全屏 | [svg](./svg/death-screens.svg) |
| **终结画面** | 终焉之言 + 创建新角色 | 终结时全屏 | [svg](./svg/death-screens.svg) |
| **全服天劫广播** | 屏幕顶部红字 + 雷云图标 + 方向指引 | 天劫事件时 | [svg](./svg/tribulation-ui.svg) |
| **渡劫观战提示** | 50 格内自动提示前往 | 天劫事件时 | [svg](./svg/tribulation-ui.svg) |
| **NearDeath 视觉** | 视野红/模糊后处理 + "hold-on cost" 提示 | NearDeath 时 | — |
| **已学功法列表** | 品阶色 + 熟练度环 | inspect 内 | — |
| **飞行/踏空 HUD** | 剩余 qi 倒计时 + 强制下落预警 | 飞行时 | — |

---

## 2. 按原 plan 章节追溯的 UI 需求

以下小节直接对应 `plan-combat-no_ui.md` 中被标注为"客户端 UI"的部分，保留原文便于交叉查阅。

### 2.1 法术体积调控（来自 §3.5）

**施法 HUD**：
- `radius` 滑块（0.1–2.0 m），实时预览：球体大小、velocity、预估飞行距离（按当前 qi_invest 算 current_qi 归零距离）
- `qi_invest` 滑块
- 二者联动显示 "在 N 格内可命中" 提示 —— **玩家的物理直觉**：贴脸用大球，远程用细针

### 2.2 功法客户端 UI（来自 §5.5.6）

- inspect 新加 "已学功法" 列表（品阶色 + 熟练度环）
- 快捷栏支持绑定主动功法（数字键释放 / 长按维持）
- 飞行/踏空 HUD：剩余 qi 倒计时 + "强制下落预警"

### 2.3 伤口疗愈 inspect 衔接（来自 §5.6.8）

inspect 伤口层（已有骨架）显示：
- 各部位 wound 的 severity（圆圈大小）+ kind 颜色
- HealingState 图标（红=Bleeding / 黄=Stanched / 绿=Healing / 黑=Scarred）
- 感染度进度环（0→1 时高亮警告）
- Scar 永久标记（持续显示，提醒此处易复伤）

**NearDeath 视觉处理**（来自 §5.6.7）：
- 视野变红/模糊（后处理 shader 或全屏半透明红层）
- UI 层锁定不可施法/攻击提示
- 底部 "hold-on cost: 0.5 qi/s" 倒计时

### 2.4 武器与法宝 UI（来自 §6.10）

- inspect "持有武器" 槽：kind、material、quality、durability 进度条
- 法宝特殊 UI：展开看 grade、bond_strength、qi_pool、abilities 列表、prev_owners
- 修复/养护界面：拖材料/丹药 → 进度条
- 炼器面板：选 kind + 选材料 + 显示成功率预测

### 2.5 状态效果 UI（来自 §7.8）

- inspect "状态" 面板：按 kind 分组，图标 + 剩余时间条 + 叠层数
- HUD 顶部状态栏：最多 8 个最紧急效果（DoT 优先 → 控制 → 加成）
- 图标描边用 `source_color` 染色（被谁打的一目了然）
- Tooltip：来源 entity / zone / 染色 + dispel 难度 + 剩余时间精确到 0.1s

---

## 3. 目录规划

```
client/src/main/java/com/bong/client/combat/
├── CombatClientBootstrap.java       # 注册 ServerDataHandler + 按键
├── handler/
│   ├── CombatEventHandler.java      # 伤害飘字 / 命中粒子
│   ├── StatusSnapshotHandler.java   # 更新 StatusEffectStore
│   ├── DerivedAttrsHandler.java     # 飞行/虚化等状态入 store
│   ├── DeathScreenHandler.java      # 致死 → 开全屏 DeathScreen
│   └── TerminateScreenHandler.java  # 终结 → 开 TerminateScreen
├── hud/
│   ├── CombatHudOverlay.java        # HUD 顶部状态栏（8 槽）+ 真元条扩展
│   ├── StaminaBar.java              # 三状态条之一的体力条
│   ├── SpellVolumePanel.java        # radius/qi_invest 双滑块
│   ├── FlightHud.java               # 飞行/踏空剩余 qi
│   └── TribulationBroadcast.java    # 全服天劫顶部红字
├── screen/
│   ├── DeathScreen.java             # 60s 倒计时 + 重生/终结
│   ├── TerminateScreen.java         # 终焉之言 + 建新角色
│   ├── ForgeCarrierScreen.java      # 暗器制作
│   ├── ZhenfaLayoutScreen.java      # 阵法布置
│   └── RepairScreen.java            # 武器/法宝修复
├── inspect/
│   ├── WoundLayerBinding.java       # 接 Wounds[] 到现有 inspect 伤口层
│   ├── StatusPanelExtension.java    # inspect 状态面板
│   ├── TechniquesListPanel.java     # 已学功法列表
│   └── WeaponTreasurePanel.java     # 武器/法宝 tooltip + 法宝展开
└── store/
    ├── StatusEffectStore.java        # volatile 快照 + listener
    ├── DerivedAttrsStore.java
    └── WoundsStore.java              # inspect 伤口层数据源
```

---

## 4. 阶段化实施（配合 `plan-combat-no_ui.md` C1-C7）

| 阶段 | 对应 server 阶段 | UI 交付 |
|---|---|---|
| **U1** | C1 基础设施 | WoundsStore + inspect 伤口层绑定 + Stamina 条 + 基础伤害飘字 |
| **U2** | C2 完整攻击事务 | SpellVolumePanel（radius/qi_invest 双滑块）+ 状态效果顶部栏 + CombatHudOverlay 真元条扩展 |
| **U3** | C3 死亡-重生 | DeathScreen（60s 倒计时 + 遗念 + 重生/终结）+ NearDeath 视觉后处理 |
| **U4** | C4 终结归档 | TerminateScreen + 感染度进度环 + Scar 永久标记 |
| **U5** | C5 四攻三防完整 | 防御 UI（截脉弹反指示/涡流键/伪皮层）+ ForgeCarrierScreen + ZhenfaLayoutScreen + inspect 状态面板全量展开 |
| **U6** | C6 天劫 | TribulationBroadcast（全服红字）+ 渡劫观战提示 + DerivedAttrs 大图标（TribulationLocked） |
| **U7** | C7 飞行 | FlightHud（qi 倒计时 + 下落预警）+ DerivedAttrs 飞行图标 |
| **并行（任何阶段）** | — | WeaponTreasurePanel（inspect 武器/法宝）+ TechniquesListPanel（已学功法）+ RepairScreen |

---

## 5. 验收

每个 U 阶段独立验收，标准：
- 所有数据来自 server 推送（网络层可 mock 验证）
- 关闭 server 时 UI 优雅降级（显示 "——" 而非崩溃）
- `./gradlew runClient` 能直观看到对应界面
- owo-lib BaseComponent 风格一致（字体、间距、颜色与 inspect 屏对齐）

---

## 6. 与其他 plan 的关系

- **`plan-combat-no_ui.md`** — 数据 / schema 权威，本文档跟随
- **`plan-HUD-v1.md`** — 常驻 HUD 基础（两层快捷栏、三状态条、事件流），本文档扩展战斗相关 overlay
- **`plan-inventory-v1.md`** — inspect 屏容器，本文档在其 tab 内插入武器/法宝/状态/功法面板
- **`plan-cultivation-v1.md §7`** — 修炼 UI（经脉层、突破闭关、顿悟、淬炼），与本文档并列互不侵入
