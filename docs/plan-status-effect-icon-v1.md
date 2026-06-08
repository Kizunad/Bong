# Bong · plan-status-effect-icon-v1 · 骨架

状态效果 HUD 图标补全。当前 `StatusEffectHudPlanner` 只画纯色色块（`rect()` 边框 + tint 填充），没有渲染任何纹理图标——plan-alchemy-combat-v1 设计时提到"图标 source_color 描边"但实施只落地了色块。本 plan 补全整条链路：server 发 icon 字段 → client 解析 → HudPlanner 渲染纹理 → gen-image 批量生成图标 PNG。

**世界观锚点**：`worldview.md` §四 战斗（状态效果是战斗/丹药/修炼三条线的 HUD 出口）

**交叉引用**：`plan-alchemy-combat-v1`（status_snapshot emit + StatusEffectHudPlanner 首次实装）· `plan-cultivation-pacing-v1`（CultivationAcceleration 等新 kind）· `plan-combat-no_ui.md`（HudRenderCommand 体系）

---

## 接入面 Checklist

- **进料**：`server/src/combat/events.rs::StatusEffectKind`（35 个变体）→ `server/src/network/status_snapshot_emit.rs`（wire shape）
- **出料**：`client/src/main/resources/assets/bong-client/textures/hud/status_effects/*.png`（图标资产）→ `StatusEffectHudPlanner`（HUD 渲染）
- **共享类型**：复用 `HudRenderCommand::texture()`（已有，`TEXTURED_RECT` kind）；复用 `StatusEffectStore.Effect` record（加 `iconId` 字段）
- **跨仓库契约**：
  - server：`status_snapshot_emit.rs` payload 新增 `"icon"` 字段（string，effect id）
  - client：`StatusSnapshotHandler` 解析 `"icon"` → `StatusEffectStore.Effect` 新增 `iconId`
  - client：`StatusEffectHudPlanner` 用 `HudRenderCommand.texture()` 渲染 `bong-client:textures/hud/status_effects/{iconId}.png`
- **worldview 锚点**：§四 战斗（状态效果可视化）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| **P0** ⬜ | wire shape 扩展 + client 解析 + HudPlanner 纹理渲染 | ⬜ |
| **P1** ⬜ | gen-image 批量生成 35 种状态效果图标 PNG | ⬜ |
| **P2** ⬜ | 饱和测试 + 视觉验收 | ⬜ |

---

## P0：wire shape + client 渲染管线

### Server 侧

1. `status_snapshot_emit.rs`：每条 effect JSON 新增 `"icon": status_effect_icon_id(&effect.kind)` 字段
2. 新增 `fn status_effect_icon_id(kind: &StatusEffectKind) -> &'static str`：
   - 大多数 kind 直接取 `status_effect_id()` 的返回值作为 icon 文件名（`bleeding`、`slowed`、`stunned` 等）
   - 参数化 kind（`BodyPartResist(part)`、`BodyPartWeaken(part)`、`AlchemyBuff(tag)`）归类到少量通用图标（`body_resist`、`body_weaken`、`alchemy_generic`）
   - 不新增 enum / struct，纯 match → &str

### Client 侧

3. `StatusEffectStore.Effect` record 新增 `String iconId` 字段（compact canonical name = 不含路径前缀的文件名 stem）
4. `StatusSnapshotHandler`：解析 `"icon"` 字段填入 `iconId`；缺失时 fallback 空字符串
5. `StatusEffectHudPlanner.buildCommands()`：border + background 之后、stack count 之前插入：
   ```java
   if (!e.iconId().isEmpty()) {
       // 有图标：渲染 16×16 PNG 缩放到 14×14 内区，不画 tint rect
       String path = "bong-client:textures/hud/status_effects/" + e.iconId() + ".png";
       out.add(HudRenderCommand.texture(
           HudRenderLayer.STATUS_EFFECTS,
           path, x + 2, y + 2,
           SLOT_SIZE - 4, SLOT_SIZE - 4,
           0xFFFFFFFF
       ));
   } else {
       // 无图标 fallback：画 tint rect（兼容旧 payload / 未知 kind）
       out.add(HudRenderCommand.rect(..., tintForKind(e.kind())));
   }
   ```

### 测试

- server：`status_snapshot_emit::tests` 新增 icon 字段断言（每个 category 至少一个 kind 验 icon 值）
- server：参数化 kind（BodyPartResist/Weaken、AlchemyBuff）→ 通用 icon id 断言
- client：`StatusEffectHudPlannerTest` 验证有 iconId 时产出 `TEXTURED_RECT` command、无 iconId 时 fallback 纯色块
- client：`StatusSnapshotHandlerTest` 验证 `"icon"` 字段解析 + 缺失兼容

---

## P1：gen-image 批量生成图标

用 `/gen-image item` 风格批量生成 status effect 图标 PNG。

### 图标设计规范

- 尺寸：16×16 px（与 SLOT_SIZE - 4 = 14 渲染区适配，PNG 稍大留 1px padding）
- 风格：水墨 icon 风格（对齐项目整体美术风格），单色主体 + 透明背景
- 不需要边框（HudPlanner 已有 border rect）
- 每种 category 图标带轻微色调倾向：buff 偏青绿、debuff 偏橙黄、dot 偏暗红、control 偏紫

### 图标清单（按 category 分组）

**DOT（1）**：`bleeding`（滴血）

**Control（6）**：`stunned`（僵直）· `vortexcasting`（涡旋）· `parryrecovery`（收势）· `staggered`（反震）· `disoriented`（迷乱）· `voidcoreactive`（坍缩）

**Buff（14）**：`damagereduction`（盾）· `breakthroughboost`（升星）· `anti_spirit_pressure_pill`（抗压）· `qi_regen_boost`（回气↑）· `insightflash`（顿悟闪）· `wound_heal`（伤口愈合）· `body_resist`（部位硬化·通用）· `speed_boost`（疾行）· `stamina_recov_boost`（回力）· `mirror_concealment`（镜隐）· `sword_parrying`（剑格）· `spirit_treasure_perception`（灵宝感知）· `cultivation_acceleration`（修炼加速）· `extraordinary_meridian_acceleration`（奇经加速）

**Debuff（16）**：`slowed`（迟缓）· `damage_amp`（伤害放大）· `humility`（谦抑）· `insight_hallucination`（幻觉）· `frailty`（风烛）· `qi_cap_perm_minus`（真元折损）· `contamination_boost`（丹毒）· `body_weaken`（部位脆弱·通用）· `stamina_crash`（虚脱）· `qi_drain_for_stamina`（真元换体）· `leg_strain`（腿伤）· `qi_regen_paused`（真元停滞）· `mirror_exposed`（镜照暴露）· `resonance_locked`（共振锁定）· `qi_regen_slowed`（回气减速）· `damage_vulnerability`（脆弱易伤）

**Unknown（1）**：`alchemy_generic`（丹药通用 fallback）

**总计：38 张 PNG**

### 文件路径

```
client/src/main/resources/assets/bong-client/textures/hud/status_effects/
├── bleeding.png
├── stunned.png
├── slowed.png
├── ...
└── alchemy_generic.png
```

---

## P2：饱和测试 + 视觉验收

1. **wire shape pin 测试**：全 35 个 StatusEffectKind 变体 → icon id 逐一断言（server）
2. **client 渲染 round-trip**：mock status_snapshot payload（含 icon 字段）→ StatusEffectStore → HudPlanner → 验 TEXTURED_RECT command 数量 + texturePath 正确
3. **纹理存在性断言**：遍历 server 所有 icon id → 对应 client PNG 文件必须存在（build-time 或 test-time 校验）
4. **视觉验收**：runClient 进游戏 → `/technique add` + 触发各种 buff/debuff → 截图确认图标渲染正确、边框色正确、tint 叠加不遮挡图标
5. **兼容性**：旧版 server payload（无 icon 字段）→ client 不崩、fallback 纯色块

---

## §8 开放问题（P0 决策门前需收口）

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

### #1 图标尺寸与渲染区适配

SLOT_SIZE=18，border 2px，内区 14×14。PNG 源应该 16×16（对齐 MC 标准纹理）还是 14×14（精确填满）？16×16 更通用但渲染时会缩放。

### #2 tint rect 是否保留

有图标后 tint 色块（`0x80XXXXXX`）是保留在图标下方（做分类氛围底色），还是去掉只留 border 描边？保留的好处是即使图标辨识度不够，玩家也能从底色一眼分辨 buff/debuff/dot/control。

### #3 AlchemyBuff(tag) 的 icon 策略

当前 `AlchemyBuff(String)` 是动态 tag（丹药副效），无法预生成每个 tag 的图标。fallback 到 `alchemy_generic.png` 是否够用，还是需要按 tag 前缀做少量分类图标？

---

## §8.1 决议（pre-P0 收口，2026-05-24）

### #1 图标尺寸

**决议**：
1. PNG 源统一 16×16，渲染时 `drawTexture` 缩放到 14×14 内区（SLOT_SIZE - 4）
2. 16×16 对齐 MC 标准纹理尺寸，gen-image 产出更通用；1px 缩放在 HUD 尺度无视觉损失

**落点**：`client/src/main/java/com/bong/client/hud/StatusEffectHudPlanner.java:69-71`（texture command 的 width/height 参数）· plan P0 §5

### #2 tint rect 去掉

**决议**：
1. 有图标时**不画 tint rect**，只保留 border（`source_color`）+ background（`TRACK_BG`）+ 图标 + progress bar
2. 无图标（iconId 为空）时仍 fallback 画 tint rect（兼容旧 payload / 未知 kind）
3. 分类辨识依靠 border 颜色（已按 category 着色：绿=buff、橙=debuff、红=dot、紫=control）+ 图标本身

**落点**：`client/src/main/java/com/bong/client/hud/StatusEffectHudPlanner.java:68-71`（tint rect 改为条件渲染）· plan P0 §5

### #3 AlchemyBuff fallback

**决议**：
1. `AlchemyBuff(tag)` 统一 fallback 到 `alchemy_generic.png`，不按 tag 分类
2. 丹药副效种类动态增长，预生成分类图标维护成本高且收益低；`alchemy_generic` + border 颜色足够区分

**落点**：`server/src/network/status_snapshot_emit.rs` `fn status_effect_icon_id`（AlchemyBuff match arm → `"alchemy_generic"`）· plan P1 图标清单

---

## §10 实施工作流

### §10.1 PR 序列

| PR | 内容 | 依赖 |
|----|------|------|
| PR-1 | P0 wire shape + client 渲染管线 + 单测 | 无 |
| PR-2 | P1 gen-image 批量生成 38 张图标 + P2 饱和测试 + 视觉验收 | PR-1 |

### §10.2 subagent 配置

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...任务...\n\nultrathink"
)
```

### §10.3 CodeRabbit 等待

每 PR 按 `docs/CLAUDE.md` §6.5 ScheduleWakeup 1200s 协议。

### §10.4 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan plan-status-effect-icon-v1` 后全自动：PR-1 → merge → PR-2 → merge → 归档。
