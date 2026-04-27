# Bong · plan-HUD-v1

**HUD 设计专项**。从 `plan-combat-v1.md §12` 抽离并深化，定义 Bong 客户端全部 HUD（渲染层 A）元素的布局、渲染规则、数据契约、键位、状态机与实施节点。

**落位**：`client/src/main/java/com/bong/client/hud/`（`HudRenderCallback` 注册层）· owo-lib 图形 Component · 不走 BaseOwoScreen。

**交叉引用**：
- 视觉草图 → `docs/svg/hud-combat.svg`
- UI 架构总览 → `docs/svg/ui-architecture.svg`
- 详细面板（非 HUD）→ `plan-combat-v1.md §12` 引用的各张 `docs/svg/inspect-*.svg`

---

## §0 设计轴心

| 原则 | 含义 | 反模式（禁止） |
|---|---|---|
| **沉浸式极简** | HUD 常驻元素尽可能少，Tarkov / Mount&Blade / Kenshi 风格 | 像 MMO 一样铺满血条、buff 槽、debuff 槽、技能盘 |
| **条件渲染优先** | 元素只在玩家真的需要时出现，结束即消失 | 「灰掉/半透明占位」— 对一个不可用的图标长期占屏幕 |
| **图形直觉胜过数字** | 用剪影/颜色/位置表达状态，尽量不显示裸数字 | `qi 220/280` 这种精确数值在 HUD 上 |
| **反馈优先用音效/边缘闪烁** | 「敌袭」「低血」通过屏幕边缘脉动 + 音效传递 | 为每个事件加一个 UI 面板 |
| **详细数字统一进 InspectScreen（I 键）** | 策略玩家可以随时按 I 看精确值 | 为「看伤口严重度 9.3」在 HUD 上加文字 |
| **未解锁/未装备一律不渲染** | 不要泄露未解锁内容，不要占空间 | 灰掉的流派指示器 |

---

## §1 总览布局

### 1.1 屏幕分区

```
┌────────────────────────────────────────────────────────────────┐
│                                                   [MC小地图/F3]│
│                                                                 │
│                                                                 │
│                [屏幕中心条件：截脉弹反环 200ms]                │
│                                                    ┌──────────┐│
│                                                    │ 事件流   ││
│                                                    │ (常驻)   ││
│                                                    │ 多源合流 ││
│                                                    │          ││
│                                                    └──────────┘│
│                                                                 │
│                                                                 │
│                                                                 │
│┌────────┐                                                       │
││人体+双条│               [F1][F2][F3][F4][F5][F6][F7][F8][F9]  │ ← 900
│└────────┘               [ 1 ][ 2 ][ 3 ][ 4 ][ 5 ][ 6 ][7][8][9]│ ← 980
│ (常驻)                                           [条件:法术体积]│
│                  (中下两层快捷栏，常驻)           (右下，按R显)│
└────────────────────────────────────────────────────────────────┘
```

### 1.2 常驻白名单（永远显示）

| 区域 | 元素 | 详见 |
|---|---|---|
| 左下 | 迷你人体剪影 + 真元竖条 + 体力竖条 | §2.1 |
| 中下（上层） | 快捷使用栏 F1-F9 | §2.2 |
| 中下（下层） | 战斗快捷栏 1-9（沿用 MC hotbar） | §2.2 |
| 右侧 | 通用事件流 | §2.3 |
| 右上 | MC 原生小地图 / F3 坐标 + 区域/灵气注解 | §2.4 |

### 1.3 条件渲染元素（详见 §3）

| 元素 | 触发条件 | 结束条件 |
|---|---|---|
| 法术体积面板（右下） | 持法术武器 AND 按住 R | 松开 R 或切换武器 |
| 截脉弹反环（屏幕中心） | server 推 `DefenseWindow`（200ms） | 窗口结束 / 已防御 / 已命中 |
| DerivedAttrs 边缘短显 | `飞行/虚化/渡劫锁定` 状态切换 | 状态离开 + 2s 淡出 |
| 伪皮层数角标 | 替尸流已解锁 AND `fake_skin.layers > 0` | 层数归零 |
| 涡流冷却角标 | 绝灵流已解锁 AND（激活中 OR 冷却中） | 冷却结束 |
| cast bar | `CastStateStore != Idle` | 完成 / 打断 |

### 1.4 明确移除（不要再出现）

| 元素 | 原因 | 替代方案 |
|---|---|---|
| 中上 8 槽状态效果面板 | 信息噪音大 · 平时无必要 | 按 I · 状态 tab（见 `inspect-status.svg`） |
| 左下防御状态文字面板 | 姿态切换有音效即可 | 音效 + 屏幕边缘短闪（§5） |
| 顶部左 DerivedAttrs 常驻大图标 | 多数时间不触发 | 触发时屏幕边缘短显（§3.3） |
| 精确数字的 HP/qi/stamina 条 | 破坏沉浸 | 人体 + 百分比竖条（§2.1） |

---

## §2 常驻元素详解

### 2.1 左下角状态小控件

**位置**：`translate(50, 820)` 左下角固定锚点，半透明深色圆角背景（opacity 0.32）托底。

**构成**：

```
┌──────────────────────────────┐
│  ○           ┃┃  ┃┃          │
│ ╱█╲  伤口×3  ┃┃  ┃┃          │
│█ ● █  (叠加红点)┃┃  ┃┃       │
│ █ █          ┃┃  ┃┃          │
│ ▌ ▌          ┃┃  ┃┃          │
│ ▌●▌          ┃┃  ┃┃          │
│ ▌ ▌          78% 42%         │
│  伤口       真元  体力        │
└──────────────────────────────┘
  (80w×150h)  (20w×130h 各)
```

**人体剪影**（80×150）：
- 灰色简化人形：头（circle r=12）+ 躯干/左臂/右臂/左腿/右腿（rect）
- 伤口叠加：按 `PhysicalBodyStore.wounds` 的 `BodyPart` 在对应位置画小圆点
- 颜色按 `WoundLevel` 枚举：
  - `INTACT` → 不画
  - `BRUISE` → `#c08040` r=2（淡棕淤青）
  - `ABRASION` → `#ffcc40` r=3（黄擦伤）
  - `LACERATION` → `#ff4040` r=5（红撕裂）
  - `FRACTURE` → `#a01818` + stroke `#ff6060` r=4（深红骨折）
  - `SEVERED` → 该部位整段变深灰描边红
- 复用现有 `BodyInspectComponent` 精简版 or 新增 `MiniBodyComponent extends BaseComponent`

**真元竖条**（20×130）：
- 青色渐变 `url(#qi)` 从底部上升
- 顶端小字 `真元` label，底端显示 `{N}%`（整数百分比）
- 低于 15% 时竖条边框闪烁

**体力竖条**（20×130）：
- 黄色渐变 `url(#stamina)`
- 同格式，显示 `{N}%`
- 低于 15% 时闪烁 + 触发 §5 边缘脉动

**数据契约**：
```rust
// server 推
pub struct CombatHudState {
    pub qi_percent: f32,        // [0.0, 1.0]
    pub stamina_percent: f32,   // [0.0, 1.0]
    pub derived: DerivedAttrFlags, // §3.3：Flying / Phasing / TribulationLocked
    // 不推裸数字，防止沉浸破坏
}
```
Channel：`bong:combat/hud_state` · 频率：60ms 节流或变化时推送。

伤口数据直接复用 `PhysicalBodyStore`（已有），无需新 channel。

### 2.2 两层快捷栏

**下层 · 战斗快捷栏（1-9）**：
- 位置 `translate(640, 980)` · 9 格 × 60px · 槽位尺寸同 MC 原生
- 沿用 MC `Inventory.selected` 机制，1-9 键即时切换
- 手持法宝时格子描边青紫色（`#c040ff`）+ 右上 ★ 标注绑定
- 选中 qi-投掷类武器时按住右键蓄力（触发 §3.1 法术体积面板）
- **无 cast time**：切换手持是立即的

**上层 · 快捷使用栏（F1-F9）**：
- 位置 `translate(640, 900)` · 9 格 × 60px · 与下层对齐
- **玩家在 InspectScreen 内拖拽配置**（详见 §10）
- F1-F9 按键发 `UseQuickSlotIntent { slot: 0..8 }`
- **有 cast time**：server 返回 `CastStateStore.active = Some { slot, duration, started_at }` 后客户端在对应格子下方渲染 cast bar（详见 §3.5 / §4）
- 冷却中的槽位蒙灰（`#555` overlay + 右下角倒计时）

**视觉区分**：
| 属性 | 战斗栏 | 快捷使用栏 |
|---|---|---|
| 尺寸 | 60×60 | 60×60 |
| 标签色 | 白/原色 | 绿字 `#80ffcc` |
| cast time | 无 | 有（§4） |
| 配置方式 | MC 原生拖拽（E 键背包） | InspectScreen I 键内 tab 拖拽 |
| 存储 | `PlayerInventory.hotbar`（MC 原生） | `QuickUseSlotStore`（新增持久化） |

### 2.3 通用事件流

**位置**：`translate(1700, 200)` 右侧竖条 · 200×420 半透明底。

**不只给战斗**。合流以下 channel（Rust server 各模块发布，client 统一订阅）：

| Channel | 事件例 | 颜色 | 图标 |
|---|---|---|---|
| `bong:combat/event` | `你对 野狼 造成 18 伤害` | `#ff6060` | ⚔ |
| `bong:cultivation/event` | `修为 +2（斩妖得悟）` | `#80ff80` | ✨ |
| `bong:world/event` | `远方雷云聚集（天劫预兆）` | `#ffff80` | 🌩 |
| `bong:social/chat` | `玩家B：一起打吗` | `#a0c0ff` | 💬 |
| `bong:system/notice` | `灵田 3 号 · 百草丹成熟` | `#80ff80` | 💠 |

每条 1 行 · 10px 字 · 右进左出滚动（FIFO 最多 18 条）。
节流/折叠规则见 §6。

**Store**：`UnifiedEventStore`（新增，client 本地合流多 channel）。

### 2.4 小地图与坐标

右上 `translate(1700, 40)`：
- **复用 MC 原生**小地图（JourneyMap / Xaeros / 或无 mod 时的 F3 debug）
- 叠加自定义 label：`坐标 X Z` / `区域：{zone_name}` / `灵气：{ling_qi_density:.2f}`
- 数据来自 `ZoneState`（已有）

**不自绘小地图**（属于 plan-worldgen / plan-client UI 其他专项）。

---

## §3 条件渲染元素

### 3.1 法术体积面板

**位置**：右下 `translate(1400, 800)` · 420×180 虚线框。

**触发**：
- `持法术武器`（WeaponCarrier.kind == Spell）AND `按住 R`

**内容**（`plan-combat-v1 §3.5`）：
- `radius`（0.3m ~ 5m）滑块
- `velocity_cap`（5 ~ 80 m/s）滑块
- `qi_invest` 预览条 + 击杀概率（server 计算）

**交互**：
- 滚轮调 radius · Shift+滚轮调 velocity_cap · 左键释放 · R 松开/取消
- 释放时发 `CastSpellIntent { radius, velocity_cap, qi_invest }`

**Store**：`SpellVolumeState`（本地，无网络）。

### 3.2 截脉弹反环

**位置**：屏幕正中央（以人物朝向前方粒子为锚）。

**触发**：server 推 `DefenseWindow { duration_ms: 200, expires_at }` payload → client 立即进入渲染。

**视觉**：
- 红色圆环从外向内收缩（200ms 匀速）
- 外圈起始半径 120px，内圈目标 40px
- 命中圆环最窄时间（100-120ms）按 `JiemaiIntent` = 极限弹反

**Channel**：`bong:combat/defense_window`（服务器→客户端）。

**结束**：窗口到期 / 玩家按键 / 实际伤害命中 — 三种情况均立即隐藏。

### 3.3 DerivedAttrs 边缘短显

不再常驻。状态切换时在屏幕边缘（顶/底/左/右择一）短显 1.5s + 轻音效。

| 状态 | 位置 | 颜色 | 持续 |
|---|---|---|---|
| 进入飞行 `Flying` | 屏幕顶部横幅 | `#80a0ff` | 1.5s |
| 进入虚化 `Phasing` | 屏幕全屏淡紫 overlay（20% opacity） | `#ff80ff` | 1s |
| 进入渡劫锁定 `TribulationLocked` | 屏幕四角红色闪烁 + 顶部文字「天劫临身」 | `#ff4040` | 常驻直到天劫结束（例外：这是紧急状态） |

> **「天劫锁定」的含义澄清**：锁定指**输入锁**——禁止战斗 Intent（攻击/移动/快捷使用）、禁止退出游戏；**不锁 Screen**——InspectScreen（I）/ CultivationScreen（K）仍可打开做只读查看（见 §13 #6）。

**Store**：`CombatHudState.derived`（统一于 §2.1 引入的 `CombatHudState`，在其中加 `derived: DerivedAttrFlags` 字段）。

### 3.4 伪皮层数 / 涡流冷却角标

小角标，位置：左下角状态小控件**下方**（不占人体图空间）。

- **伪皮**（替尸已解锁 AND `fake_skin.layers > 0`）：小方块 × N 堆叠，N = 剩余层数
- **涡流**（绝灵已解锁）：小环形进度条 · 激活中 = 蓝色转圈 · 冷却中 = 灰色倒计时

两者仅在对应流派已解锁时渲染（§1.4 规则）。

### 3.5 cast bar

**位置**：对应 F1-F9 格子下方 5px 条。

**触发**：`CastStateStore.active = Some { slot, duration_ms, started_at }`

**视觉**：
- 背景色 `#1a1000` · 进度色 `#ffcc40`
- 宽度 = 60px（对齐槽位）· 高度 5px
- 被打断时瞬间变红 `#ff4040` 再淡出（0.3s）

详见 §4 状态机。

---

## §4 cast 状态机 + 打断规则

### 4.1 三态定义

```
    ┌─────┐   UseQuickSlotIntent   ┌──────────┐
    │Idle │ ────────────────────▶  │ Casting  │
    └─────┘                        └──────────┘
       ▲                               │    │
       │                         done  │    │ interrupted
       │                               ▼    ▼
       │                         ┌────────┐ ┌─────────┐
       └─────────────────────────┤Complete│ │Interrupt│
                                 └────────┘ └─────────┘
                                      │           │
                                      └──── 返 Idle（0.3s 淡出 cast bar）
```

### 4.2 转移条件

| 事件 | 当前态 → 目标态 | 动作 |
|---|---|---|
| 按 F{1-9} | Idle → Casting | server 检查 cooldown / 物品存在 / 非眩晕；通过则 `CastStateStore.active = ...` + 向 client 回推 `CastSync { slot, duration_ms }` |
| 按 F{1-9} | Casting → （忽略，不中断） | 重复按同键不取消，但可播"咿嗯"反馈音提示正在施法 |
| 按 F{其他} | Casting → Interrupt | 主动取消；被打断的物品**不消耗**（见 4.4） |
| 达到 duration | Casting → Complete | server 应用效果（加血/解毒/去 debuff）+ 推 `CastSync { active: None, outcome: Completed }` |
| 移动位移 > threshold | Casting → Interrupt | 见 4.3 打断阈值 |
| 受击 contam > threshold | Casting → Interrupt | 同上 |
| 进入 Stun / Silenced | Casting → Interrupt | 直接取消 |
| 打开 Screen（I / K / E） | Casting 不变 | **不打断**。Screen 内可以看状态，cast 继续在后台计时；cast bar 作为 §8.2 HUD 隐藏的**例外**保留（见 §8.2 备注） |
| 玩家死亡 | Casting → Interrupt | 立即清空 |

### 4.3 打断阈值矩阵

| 打断源 | 默认阈值 | 可配置 | 例外（不打断） |
|---|---|---|---|
| 主动位移 | 位移 > 0.3m（5 tick 内累积） | `QuickSlotCastInterruptMovement` | 被动位移（被击退）算入受击打断，不双算 |
| 受击 contam | 累积 > `duration × 0.05 × max_hp`（按 5% HP / 秒 为阈值） | `QuickSlotCastInterruptContam` | contam 为 0 的纯控制攻击算控制打断 |
| 控制效果 | `Stun`, `Silenced(Physical)`, `Knockback`, `Charmed` | 固定 | `Slowed`, `DamageAmp` 不打断 |
| 主动按键 | 任意其他 F 键 or 1-9 切换武器 | 固定 | 鼠标移动 / 小地图打开 不打断 |
| 死亡 | `hp == 0` | 固定 | — |

### 4.4 打断后物品返还策略

| 结果 | 物品扣除 | 冷却 |
|---|---|---|
| `Completed`（正常完成） | 扣除 1 个 | 触发正常冷却（按物品定义） |
| `Interrupt(Movement)` | **不扣除** | 短冷却（0.5s · 防连点） |
| `Interrupt(Contam)` | **不扣除** | 短冷却（0.5s） |
| `Interrupt(Control)` | **不扣除** | 由控制效果自然持续，解除后立刻可用 |
| `Interrupt(UserCancel)` | **不扣除** | 短冷却（0.5s） |
| `Interrupt(Death)` | 不扣除（玩家已死亡） | — |

理由：打断=没吃到药/没上完药，物品仍在背包。只有 `Completed` 才消耗。short CD 0.5s 防止连点惩罚。

---

## §5 屏幕边缘反馈清单

### 5.1 反馈通道

| 通道 | 实现 | 持续 |
|---|---|---|
| **Edge Pulse** | 屏幕四周 color overlay（外圈实线 → 内圈淡出） | 单次 0.4s · 可 loop |
| **Edge Flash** | 单边短促闪一下 | 0.15s |
| **Full Tint** | 全屏薄色调（alpha < 15%） | 持续直到状态离开 |
| **Vignette** | 四角暗化加深 | 持续 |
| **Shake** | 屏幕轻微抖动（translate ±2px） | 0.1s |
| **Audio Cue** | 音效（独立于视觉） | 0.3-1s |

### 5.2 触发表

| 事件 | 视觉通道 | 颜色 | 音效 |
|---|---|---|---|
| HP < 30% | Edge Pulse loop（呼吸） | `#ff4040` | 心跳声 loop |
| HP < 10% | Edge Pulse 快速 loop + Vignette | 同上 + 暗角 | 急促心跳 |
| qi 枯竭（`qi_percent < 5%`） | Edge Flash 单次 | `#40d0d0` | 清脆"叮" |
| stamina 枯竭 | Edge Flash 单次 | `#d0d040` | 喘息声 |
| 敌袭即将命中（服务器预告 < 0.3s） | Edge Pulse 单次 快速 | `#ffcc40` | 风声 |
| `DefenseWindow` 激活（200ms） | Shake 单次 + 中心环（§3.2） | `#ff8080` | 金属碰撞音 |
| 弹反成功 | Edge Flash 四周同时闪 | `#ffff80` | 清响 |
| cast bar 被打断 | Edge Flash 四周单次（受击打断时偏向受击源方向，其他打断源无方向） | `#ff4040` | 中断音 |
| 进入虚化 `Phasing` | Full Tint 持续 | `#ff80ff` 12% | — |
| 天劫锁定 `TribulationLocked` | Vignette 深色 + 四角 Edge Pulse loop | `#ff4040` | 雷声远雷 |
| 受击 contam 触发伤口 | Edge Flash（方向按受击源） | `#ff4040` | 伤口音 |
| 进入战斗（首次命中） | Edge Flash 单次 | `#ffcc40` | 紧迫音 |

### 5.3 优先级叠加策略

多事件同时触发时按优先级叠加，而非覆盖：

```
Vignette (持续态)   →  底层
Full Tint          →  第 2 层
Edge Pulse loop    →  第 3 层
Edge Pulse 单次    →  第 4 层
Edge Flash         →  第 5 层
Shake              →  顶层（对位置做微扰）
```

同一通道同色同来源去重（避免多次受击同一方向堆叠闪烁过亮）。

---

## §6 事件流节流/折叠规则

### 6.1 节流（per channel）

| Channel | 每秒上限 | 策略 |
|---|---|---|
| `bong:combat/event` | 8 条/秒 | 超额丢弃最旧 |
| `bong:cultivation/event` | 3 条/秒 | 超额合并 |
| `bong:world/event` | 3 条/秒 | 超额丢弃 |
| `bong:social/chat` | 无上限 | 走独立聊天栏（§9） |
| `bong:system/notice` | 2 条/秒 | 超额合并 |

### 6.2 折叠（同类事件）

同 `event_kind` + 同 `source_tag` 在 **1.5s 窗口**内连续发生时折叠成一条，显示 `{原文} ×N`：

- 例：`流血 tick 3 次` → `你受到流血伤害 ×3`
- 例：`你命中 野狼` 连击 → `你命中 野狼 ×5`

**折叠规则**：
- 最多叠 99，超出显示 `×99+`
- 折叠后 N 继续递增时更新现有行（不新增行）
- 折叠行过期（1.5s 后无新增）即"落定"，不再叠加

### 6.3 优先级与时间窗口

- **最大展示**：18 条（超出从顶端挤出 + 淡出 0.5s）
- **单条存活**：6 秒（过期淡出）
- **优先级**（溢出时谁先走）：
  - P0 `critical`（死亡、击杀、渡劫预兆）— 常驻到手动关闭
  - P1 `important`（大伤害、暴击、Buff/Debuff 施加）— 6s
  - P2 `normal`（普通命中、tick 聚合后） — 4s
  - P3 `verbose`（灵田 tick、自然恢复）— 2s

### 6.4 隐藏事件流（TODO · 未来工作）

> 未在 v1 范围内，留作后续 task：
> - 设置面板加 `hud.event_stream.visible` 开关（默认开）
> - 绑定一个 toggle 键（默认未绑定）
> - 隐藏后事件仍在 `UnifiedEventStore` 累积，重新打开即可看到滚动（或选择清空）
> - 适用场景：截图 / 录制 / 纯探索时想要干净画面

---

## §7 键位全表 + Intent 映射

### 7.1 常驻键

| 键 | 作用 | 发出的 Intent / 行为 | 触发 Screen |
|---|---|---|---|
| `1`-`9` | 切换战斗栏手持 | MC 原生 `Inventory.selected = N-1` | — |
| `F1`-`F9` | 触发快捷使用槽 | `UseQuickSlotIntent { slot: 0..8 }` | — |
| `E` | 打开背包（MC 原生） | MC InventoryScreen | B |
| `I` | 打开 InspectScreen | 客户端本地 | B |
| `K` | 打开 CultivationScreen（过渡，后续废弃） | 客户端本地 | B |
| `Esc` | 关闭当前 Screen / 打开菜单 | MC 原生 | — |

### 7.2 战斗 / 施法键

| 键 | 作用 | Intent |
|---|---|---|
| 左键 | 攻击 / 释放蓄力 | `AttackIntent` / `CastSpellIntent`（取决于手持） |
| 右键 | 蓄力 / 使用物品 | `ChargeIntent { slot }` |
| `R`（按住） | 法术体积调控（§3.1） | 本地 `SpellVolumeState` 更新，松开或左键释放时与 CastSpellIntent 合并 |
| 鼠标滚轮（按住 R 时） | 调整 radius | 本地 |
| `Shift+滚轮`（按住 R 时） | 调整 velocity_cap | 本地 |
| 防御键（默认 `V`）（仅在 `DefenseWindow` 内） | 弹反**反应键**（有默认） | `JiemaiIntent` |
| `Shift+Q` | 投掷暗器 | `ThrowAnqiIntent` |
| `T` | 发动技能（ForgeWeapon / Zhenfa） | `ActivateTechniqueIntent` |

### 7.3 键位冲突检查清单

- F1-F9 占快捷使用栏 → 原 MC 的「帮助 / 截图」等调整 keymap 或移动到其他组合键
- R 占蓄力 → 与 MC 原生「R 键 = reload（某些 mod）」可能冲突，需在 KeyBinding 优先级内处理
- E / I / K 保持原位
- 所有按键通过 `KeyBinding` 系统注册，允许玩家在设置里重映射

---

## §8 HUD 特殊场景行为

### 8.1 玩家死亡

- **全部 HUD 元素**立即隐藏 / 淡出（0.3s）
- 打开 `DeathScreen`（见 `plan-combat-v1 §8` + `death-screens.svg`）
- 死亡画面内部不显示任何原 HUD 元素
- 重生 / 创建新角色后，HUD 重新出现（按 `CombatHudState.active` 判断）

### 8.2 Screen 打开（I / K / E / 其他）

| Screen 类型 | HUD 元素行为 |
|---|---|
| 背包（E） | 两层快捷栏保留 · 其他 HUD 半透明 50% · 事件流保留 |
| InspectScreen（I） | 全部 HUD 隐藏（Screen 内部有自己的状态展示） |
| CultivationScreen（K） | 同 I |
| DeathScreen / TerminateScreen | 全隐藏 |
| 其他硬编码 Screen（ForgeWeaponScreen 等） | 全隐藏 |
| Dynamic XML Screen（阵法布置等） | 全隐藏 |
| MC 原生菜单（Esc） | 全隐藏（MC 会自动 pause） |

实现：`HudRenderCallback` 入口查 `MinecraftClient.currentScreen`，按类型决定渲染策略。

**例外**：`CastStateStore.active != None` 时，cast bar 在 InspectScreen / CultivationScreen / Dynamic XML Screen 下仍然渲染（贴屏幕底部中央一小条），让玩家看得到后台计时。DeathScreen / MC 原生菜单不例外（玩家已脱离战斗上下文）。

### 8.3 断线重连

- 断线时：HUD 全部清空，显示"连接中..."（MC 原生）
- 重连时：
  1. server 首帧推 `CombatHudState` + `PhysicalBodyStore` + `QuickUseSlotStore` + `CastStateStore` + `UnlockedStylesStore` + `UiOpenState` 作为 **hydration payload**（若断线时玩家正在 cast，`CastStateStore.active.started_at` 为 server 时间戳，client 据此续算剩余时间）
  2. client 所有 Store `.replace()` 后 HUD 自然恢复
  3. 事件流不恢复历史条目（新开始累积）

### 8.4 MC 原版菜单（Esc）

由 MC 引擎处理暂停，HUD 自然不渲染。

---

## §9 narration 与事件流

### 9.1 双通道设计

Bong 有两条平行的文字通道，职责不同：

| 通道 | 存储 | 视觉 | 用途 |
|---|---|---|---|
| **MC 原生聊天栏**（下部） | `ChatHud` (MC) | 白/带色文字行 | 玩家间聊天 · 天道 narration · 系统重要公告 |
| **UnifiedEventStore 事件流**（右侧 HUD） | `UnifiedEventStore` (client) | 带图标的滚动条目 | 玩家个人视角的事件（受击/修为/灵田等） |

### 9.2 路由规则

| 事件类型 | 走聊天栏 | 走事件流 | 备注 |
|---|---|---|---|
| 玩家间聊天 `/say` 等 | ✓ | — | MC 原生 |
| 天道 narration（长叙述） | ✓ | — | 1-3 段文字，需要玩家细读 |
| 系统重要公告（全服 / 跨域） | ✓ | ✓（同时） | 例如渡劫广播 |
| 战斗事件（伤害/被击） | — | ✓ | 节流折叠 |
| 修为 / 突破 | — | ✓ | |
| 灵田 tick | — | ✓ | 低优先级 |
| 死亡结算（本人） | — | — | DeathScreen 专属 |

实现细节：
- 天道 narration 直接发送到 MC ChatHud（`client.inGameHud.chatHud.addMessage(...)`）
- `UnifiedEventStore` 只订阅上述"事件流专属"channel
- 跨域事件（如全服天劫）同时发 ChatHud + EventStore，加标记避免玩家疑惑"发了两遍"

### 9.3 重复抑制

- 聊天栏内 narration 自带 MC 原生去重（相同文本 5s 内不重复）
- 事件流按 §6.2 折叠规则处理

---

## §10 快捷使用栏配置 tab

### 10.1 入口

`InspectScreen`（I 键）新增 tab：`[装备] [修仙] [伤口] [状态] [快捷使用]`（第五个 tab）。

Tab 标识 · 键位 `Shift+Q` 或直接点击切换。

### 10.2 布局草图

```
┌──── InspectScreen · 快捷使用 tab ─────────────────────┐
│                                                       │
│  ┌─ 当前快捷栏（F1-F9） ─────────────────────────┐  │
│  │ [F1 绷带] [F2 回血丹] [F3 回真元] [F4 解毒] [F5空]│  │
│  │ [F6 空]   [F7 空]     [F8 空]     [F9 空]        │  │
│  └───────────────────────────────────────────────┘  │
│                                                       │
│  ┌─ 可配置物品（按背包筛选） ───────────────────┐  │
│  │  绷带 ×12 · 2.5s · 小伤口立止血                │  │
│  │  回血丹 ×3 · 1.2s · HP+30                      │  │
│  │  回真元丹 ×8 · 0.8s · qi+50                    │  │
│  │  解毒药 ×4 · 1.5s · 清除 Poison                │  │
│  │  导真元 · 3s · HP/伤口恢复速率 ×2（30s）       │  │
│  │  止血散 ×2 · 2s · 清除 Bleeding                │  │
│  │  ...                                             │  │
│  └───────────────────────────────────────────────┘  │
│                                                       │
│  操作：                                                │
│    · 拖拽物品 → 快捷栏格子 = 绑定                     │
│    · 拖拽格子物品 → 外部 = 清空槽位                   │
│    · 右键格子 = 清空                                  │
│    · 只能绑定 `ItemKind == Consumable` 的物品         │
│    · 重复绑定同一物品到不同槽位 = 允许                │
└───────────────────────────────────────────────────────┘
```

### 10.3 拖拽流程

1. 玩家从「可配置物品」列表拖起物品（client `DragState.source = ItemList`）
2. 拖入某 F 槽（client `DragState.target = QuickSlot(n)`）
3. client 立即本地更新 `QuickUseSlotStore.slots[n] = ItemRef { item_id, cast_duration_ms }`
4. client 发 `QuickSlotBindIntent { slot: n, item_id }` 给 server
5. server 验证（物品属 consumable + 玩家背包内确实有该物品）→ 回 `QuickSlotConfirm { slot, ok }`
6. 若 server 拒绝，client 回滚 Store 并弹提示

### 10.4 持久化 payload

```rust
// Client → Server
pub struct QuickSlotBindIntent {
    pub slot: u8,          // 0-8
    pub item_id: Option<ItemId>,  // None = 清空
}

// Server → Client（hydration / 其他端变更时）
pub struct QuickSlotConfig {
    pub slots: [Option<QuickSlotEntry>; 9],
}
pub struct QuickSlotEntry {
    pub item_id: ItemId,
    pub display_name: String,
    pub cast_duration_ms: u32,
    pub cooldown_ms: u32,
    pub icon_texture: String,
}
```

Channel：
- `bong:combat/quickslot_bind`（client→server）
- `bong:combat/quickslot_config`（server→client）

**server 权威存储**：玩家配置存玩家数据（与背包、修炼同级），跨会话持久化。

---

## §11 数据契约 + Store 总览

### 11.1 新增 Store（本 plan 引入）

| Store | 职责 | 消费 channel |
|---|---|---|
| `CombatHudState` | qi_percent / stamina_percent 百分比 | `bong:combat/hud_state` |
| `QuickUseSlotStore` | F1-F9 配置（[Option<QuickSlotEntry>; 9]） | `bong:combat/quickslot_config` |
| `CastStateStore` | 当前 cast 状态机状态（Idle / Casting / Complete / Interrupt + slot + progress） | `bong:combat/cast_sync` |
| `SpellVolumeState` | 本地 radius / velocity_cap / qi_invest（无网络） | — |
| `UnifiedEventStore` | 合流多 channel 事件条目（带节流/折叠） | 见 §2.3 |
| `UnlockedStylesStore` | 已解锁流派（用于条件渲染门禁） | `bong:cultivation/unlocks_sync` |

### 11.2 复用现有 Store

| Store | 本 plan 用途 |
|---|---|
| `PhysicalBodyStore` | §2.1 伤口叠加 |
| `PlayerStateStore` | 基础信息 / MC 原生血条同步 |
| `ZoneState` | §2.4 区域名 / 灵气 |
| `InventoryStateStore` | §10 可配置物品列表 |
| `DragState` | §10 拖拽 |

### 11.3 Intent 总览（client → server）

| Intent | 触发键 | Payload |
|---|---|---|
| `UseQuickSlotIntent` | F1-F9 | `{ slot: u8 }` |
| `QuickSlotBindIntent` | I 键 UI 拖拽 | `{ slot: u8, item_id: Option<ItemId> }` |
| `CastSpellIntent` | 左键（持法术） | `{ radius, velocity_cap, qi_invest }` |
| `JiemaiIntent` | 防御键 `V`（DefenseWindow 内） | `{}` · channel `bong:combat/jiemai` |
| `ChargeIntent` | 右键按住 | `{ slot }` |
| `ThrowAnqiIntent` | Shift+Q | `{ target_hint }` |
| `ActivateTechniqueIntent` | T | `{ technique_id }` |

### 11.4 Channel 总览

| Channel | 方向 | 频率 |
|---|---|---|
| `bong:combat/hud_state` | server → client | 60ms 节流 |
| `bong:combat/cast_sync` | 双向 | 事件驱动 |
| `bong:combat/quickslot_config` | server → client | 变更时 |
| `bong:combat/quickslot_bind` | client → server | 事件 |
| `bong:combat/defense_window` | server → client | 事件（200ms 生命周期） |
| `bong:combat/jiemai` | client → server | 事件（反应键触发） |
| `bong:combat/event` | server → client | 节流 8/s |
| `bong:cultivation/event` | server → client | 节流 3/s |
| `bong:cultivation/unlocks_sync` | server → client | 变更时（**非事件流**，喂 `UnlockedStylesStore` 做条件渲染门禁） |
| `bong:world/event` | server → client | 节流 3/s |
| `bong:social/chat` | 双向 | 原生聊天 |
| `bong:system/notice` | server → client | 节流 2/s |

---

## §12 实施节点（按 C 阶段）

| 阶段 | 状态 |
|---|---|
| C1 · HUD 骨架 + 左下状态小控件 | ✅ |
| C2 · 两层快捷栏 | ✅ |
| C3 · cast 状态机 + 打断 | ✅ |
| C4 · 通用事件流 + 节流折叠 | ✅ |
| C5 · 条件渲染元素 | ✅（伪皮 / 涡流角标未做） |
| C6 · InspectScreen 快捷使用 tab | ⏳（hotbar 渲染 + Store hydrate 已做，拖拽 tab 未做） |
| C7 · 特殊场景 | ✅ |
| C8 · 调参 + QoL | ⏳（HudConfig 已有，事件流隐藏 toggle 未做） |

### C1 · HUD 骨架 + 左下状态小控件 ✅

- [x] 新增 `HudRenderCallback` 注册点 + `MiniBodyComponent`（`MiniBodyHudPlanner`）
- [x] 新增 `CombatHudState` Store + `bong:combat/hud_state` payload（`CombatHudStateStore` / `CombatHudStateHandler`）
- [x] 左下角人体 + 双竖条渲染（百分比）（`StaminaBarHudPlanner` 等）
- [x] 伤口复用 `PhysicalBodyStore`（已有）

### C2 · 两层快捷栏 ✅

- [x] 下层：扩展 MC 原生 hotbar 描边（法宝紫 + qi_invest 指示）（`WeaponHotbarHudPlanner`）
- [x] 上层：新增 `QuickUseSlotStore` + 9 格自定义 Component（`QuickBarHudPlanner` / `QuickUseSlotStore`）
- [x] F1-F9 KeyBinding 注册 → `UseQuickSlotIntent`（`CombatKeybindings`）
- [x] cast bar 渲染（对应格子下方）

### C3 · cast 状态机 + 打断 ✅

- [x] `CastStateStore` 新增（`CastState` / `CastStateStore` / `CastSyncHandler`）
- [x] server 端 cast 管理（duration / cooldown / interrupt 检测）
- [x] 打断矩阵（移动/受击/控制/主动）（`CastInterruptRules`）
- [x] 物品返还策略（`CastOutcome`）

### C4 · 通用事件流 + 节流折叠 ✅

- [x] `UnifiedEventStore` + 多 channel 订阅（`UnifiedEventStore` / `UnifiedEventStream` / `EventStreamPushHandler`）
- [x] 节流 / 折叠算法
- [x] 右侧竖条 Component（`EventStreamHudPlanner`）

### C5 · 条件渲染元素 ✅（部分）

- [x] 截脉弹反环（中心 Canvas Component）（`JiemaiRingHudPlanner` / `DefenseWindowHandler`）
- [x] 法术体积面板（按 R 触发）（`SpellVolumeHudPlanner` / `SpellVolumeStore`）
- [x] 屏幕边缘反馈系统（Edge Pulse / Flash / Tint / Shake / Vignette）（`EdgeFeedbackHudPlanner`）
- [x] DerivedAttrs 边缘短显（`DerivedAttrIconHudPlanner` / `FlightHudPlanner` / `TribulationBroadcastHudPlanner`）
- [ ] 伪皮 / 涡流 角标（未实装：尚无 fake_skin / vortex_cooldown 数据流）

### C6 · InspectScreen 快捷使用 tab ⏳

- [ ] InspectScreen 新增第 5 个 tab（当前仅 3 tab：装备 / 修仙 / 技艺；hotbar + quick-use 已作为左侧渲染区显示，但**无独立 tab**也**无拖拽配置 UI**）
- [ ] 拖拽流程 + `QuickSlotBindIntent`（数据契约存在 `QuickSlotBindIntent.java`，但 client 未发送）
- [x] server 端权威存储 + hydration（`QuickSlotConfigHandler` / `hydrateQuickUseFromStore`）

### C7 · 特殊场景 ✅

- [x] `HudRenderCallback` 内的 Screen 类型分发（`ScreenHudVisibility`：VISIBLE / HIDDEN / CAST_BAR_ONLY）
- [x] 死亡 / 断线 二场景
- [x] narration 双通道路由（`NarrationHandler` / `NarrationState`：天道 narration → ChatHud；事件 → UnifiedEventStore）

### C8 · 调参 + QoL ⏳

- [x] 阈值可配置（`HudConfig`）
- [ ] 事件流隐藏 toggle（§6.4 TODO）

---

## §15 进度日志

- 2026-04-25：C1-C5（除伪皮/涡流角标）+ C7 + C8 阈值配置已实装；C6 仅 hotbar 渲染与 hydrate 完成，拖拽 tab 未做；伪皮/涡流角标与事件流隐藏 toggle 仍待。

---

## §13 已定案（原开放问题）

| # | 问题 | 结论 |
|---|---|---|
| 1 | 观战 HUD | **无观战功能预期**，不做（§8 已删除观战小节） |
| 2 | 色盲模式 | 不做 |
| 3 | HUD 缩放 / 无障碍 | 不做（沿用 MC 原生 GUI scale） |
| 4 | 事件流隐藏 | **允许**（开关 + 可绑定 toggle 键），v1 TODO 未来写（§6.4） |
| 5 | 天劫锁定期间 | **允许**打开 InspectScreen（状态展示而非输入锁定） |
| 6 | cast 期间打开 Screen | **不打断**，cast 继续后台计时（§4.2） |
| 7 | 防御姿态指示器 | **不做**，防御姿态不进入 HUD 范畴（切换靠音效 + §5 边缘短闪反馈） |

---

## §14 验收标准（HUD 闭环）

- ✓ 左下状态小控件在玩家连接后 1 秒内出现，随时 reflect qi/stamina/伤口
- ✓ 两层快捷栏显示 · F1-F9 可拖配置并 cast · 1-9 可切换武器
- ✓ cast bar 在使用快捷栏时出现，移动 > 0.3m 或受击 > 阈值时打断 + 物品不扣
- ✓ 屏幕边缘脉动在低血 / 敌袭 / DefenseWindow 时按 §5 触发
- ✓ 事件流实时滚动，流血 tick 正确折叠成 ×N
- ✓ 死亡时全部 HUD 隐藏，重生后恢复
- ✓ 断线重连 3 秒内 HUD 完整恢复
- ✓ 未解锁的流派指示器完全不渲染（非灰掉）
- ✓ 精确数字仅在 InspectScreen 内可查（HUD 不显示）

---

**交叉引用**：
- `plan-combat-v1.md`（§3 攻击 · §4 防御 · §5 流派 · §7 StatusEffect · §8 死亡 · §10 天劫）
- `docs/svg/hud-combat.svg`（主 HUD 草图）
- `docs/svg/ui-architecture.svg`（三层渲染骨架总览）
- `docs/svg/defense-ui.svg` / `inspect-*.svg`（相关 Screen / 面板草图）
- `client/src/main/java/com/bong/client/ui/`（现有渲染骨架：UiOpenScreens / DynamicXmlScreen / BaseOwoScreen）
