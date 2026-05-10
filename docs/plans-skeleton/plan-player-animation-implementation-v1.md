# Bong · plan-player-animation-implementation-v1 · 骨架

玩家动画系统端到端实现。承接 `plan-player-animation-v1` ✅ finished（KosmX PlayerAnimator 库调研、可控骨骼 / easing / 双路径生产 [Java + JSON] 完整设计）—— v1 把设计落地为 gradle 集成 + 首批 15+ 动画 + server→client 触发协议。**零美术工具依赖**：纯 Java 代码 + LLM 生成 PlayerAnimator JSON。

**世界观锚点**：`worldview.md §四` 战斗是近战肉搏（零距离贴脸施法）→ 需要挥拳/掌击/抱摔动画 · `§三` 打坐/突破姿态 → 静坐/经脉发光 · `§五` 七流派各自身法姿态（爆脉沉腰 / 暗器微抬腕 / 涡流展掌）· `§七` NPC 道伥模仿玩家日常行为（砍树/挖矿/蹲伏假示好）

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §音论`（身体动 → 灵气振荡 → 视觉可见）

**前置依赖**：
- `plan-player-animation-v1` ✅ → 技术基础/可控骨骼/easing/双路径全部锚定
- `plan-vfx-v1` ✅ → VFX 事件通道可复用
- `plan-HUD-v1` ✅ → HUD 层不挡动画视野
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 攻击/受击/弹反/闪避事件垫
- `plan-cultivation-v1` ✅ → 打坐/突破事件垫

**反向被依赖**：
- `plan-baomai-v3` 🆕 active → 崩拳 5 招逐帧动画
- `plan-dugu-v2` 🆕 → 蚀针弹指/扬毒粉/自蕴吞吐动画
- `plan-zhenmai-v2` 🆕 → 弹反瞬间振臂动画
- `plan-tuike-v2` 🆕 → 蜕壳剥落动作
- `plan-woliu-v2` 🆕 → 展掌开涡/收掌闭涡动画

---

## 接入面 Checklist

- **进料**：KosmX PlayerAnimator gradle 依赖 / 可控骨骼列表 / `PlayerAnimationAccess` API / `AnimationStack.addAnimLayer` / server `AnimationTriggerEvent`（新建）
- **出料**：`BongAnimationRegistry`（client 侧动画注册表）+ `BongAnimationTrigger`（server→client CustomPayload）+ 首批 15+ PlayerAnimator JSON + Java 动画 + `client/tools/gen_*.py` 动画生成器
- **跨仓库契约**：server `animation::AnimationTrigger { player, anim_id, priority }` → client `BongAnimationRegistry.play()`

---

## §0 设计轴心

- [ ] **零 Blender/Blockbench**：全部 Java 代码 + JSON 生产线
- [ ] **多层叠加**：上半身动作 / 下半身行走 / 全身姿态 独立 priority 通道（priority 100/200/300）
- [ ] **服务端权威触发**：server 决定何时播什么动画，client 纯表演
- [ ] **animation = 表演**：不做判定、不参与网络同步、不写 component（与 VFX 原则一致）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | gradle 集成 PlayerAnimator → `./gradlew test build` 验证编译 + `BongAnimationRegistry` 骨架 + 3 个验证动画（`sword_swing_right` / `meditate_sit` / `hurt_stagger`）+ server→client `bong:animation_trigger` CustomPayload + WSLg 实跑验证 | ⬜ |
| P1 | 首批 15 动画铺量：6 战斗（拳击/剑斩/掌击/windup_charge/release_burst/弹反_stagger）+ 4 姿态（打坐/采药蹲/搜刮弯腰/潜行伏低）+ 3 NPC 行为（砍树/挖矿/蹲伏假示好——道伥 lore 闭环）+ 2 UI 衔接（背包开/关微侧身、炼丹搅拌手势） | ⬜ |
| P2 | 流派专属 stance 6 个（爆脉沉腰 / 暗器微抬腕 / 阵法展掌布符 / 毒蛊藏指捻针 / 截脉侧身蓄劲 / 涡流双掌开合 / 替尸披壳动作）+ 负重/伤口 limping walk（腿部损伤 → 移速系数对应的跛行动画） | ⬜ |
| P3 | 突破四境专属动画（醒灵→引气 初感灵气仰头 / 引气→凝脉 经脉循行周身颤 / 凝脉→固元 真元凝核抱丹 / 通灵→化虚 天地共鸣双臂展开）+ 死亡倒地/消散动画（复用 DeathSoulDissipatePlayer 配合） | ⬜ |
| P4 | 给流派 vN+1 plan 各提供 1-2 招动画联调 demo + `client/tools/gen_*.py` 生产线收口（复用已有 21 个 generator 模式）+ render_animation.py headless 批量回归测试 | ⬜ |
| P5 | 饱和化测试：每个动画 3 境界 × 2 性别 × 与行走/跳跃/游泳叠加 × 被打断恢复 + 压测（5 玩家同时播 10 动画不掉帧） | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：gradle 依赖 / `BongAnimationRegistry` / 15+ JSON / `bong:animation_trigger` channel / `client/tools/gen_*.py`
- **关键 commit**：P0-P5 各自 hash
- **测试结果**：15+ 动画 WSLg 实跑 / 叠加测试 / 压测
- **遗留 / 后续**：NPC 动画（plan-npc-skin-v1 联动）/ 第一人称手臂动画（mixin 难）
