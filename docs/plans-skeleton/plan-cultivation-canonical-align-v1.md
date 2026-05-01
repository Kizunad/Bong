# Bong · plan-cultivation-canonical-align-v1 · 骨架

把 cultivation 模块从 MVP 压缩值对齐到 worldview 正典值。**Wave 0 硬阻塞**——所有下游 plan 依赖此对齐完成,整个 100h 路径成立的物理前提。

**世界观锚点**：`worldview.md §三 修炼体系` line 100-153（六境界 + 突破条件 + 时长基线 0.5/3/8/15/25h）

**library 锚点**：`cultivation-0001 六境要录` · `cultivation-0006 经脉浅述`（十二正经 + 八奇经分布）

**交叉引用**：`plan-cultivation-v1` ✅(待对齐源) · `plan-cultivation-mvp-cleanup-v1` ✅ · `plan-gameplay-journey-v1` §I/§Q.1 Wave 0/O.1（决策来源）

---

## 接入面 Checklist

- **进料**：`server/src/cultivation/components.rs` 当前 `Realm` enum + `required_meridians()` MVP 压缩值
- **出料**：对齐后的常量表 + 重算的 XP 曲线 + 同步更新的 50+ 测试 + 全栈文案统一
- **共享类型**：复用 `Realm` enum / `MeridianId` / `BreakthroughRequest` ——**不新增类型**
- **跨仓库契约**：server cultivation/* + agent schema/cultivation.ts + client CultivationScreen.java + 所有 schema sample
- **worldview 锚点**：§三 line 100-131(突破条件) + line 133-153(时长基线)

---

## §0 设计轴心

- [ ] **不留技术债**(O.1 决策)：本 plan P0 段必须直接对齐到正典值,不允许 MVP 压缩 → 正典 v2 的二次迁移
- [ ] **51.5h 时长基线锁定**：worldview §三 line 133-153 给的 0.5/3/8/15/25h 是设计目标,XP 曲线必须满足
- [ ] **测试同步**：50+ 测试用例必须连同公式一起更新,不允许公式改了测试不改
- [ ] **不修改 worldview**：worldview 是正典,本 plan 是代码追正典,反向不允许

---

## §1 已识别差异（plan-gameplay-journey-v1 §I.1）

| 项 | 代码现状 | worldview 正典 | 对齐动作 |
|---|---|---|---|
| `required_meridians()` | `[0, 1, 4, 8, 14, 20]` | **`[1, 3, 6, 12, 12+4, 12+8]`** | 公式重写 |
| XP 曲线斜率 | 当前未校准 | 满足 0.5/3/8/15/25h | 重算 |
| `Realm` doc comment | 旧名"觉醒/引灵/凝气/灵动/虚明" | "醒灵/引气/凝脉/固元/通灵/化虚" | 注释更新 |
| `lingtian/mod.rs` 头注释 | "不含补灵/收获/偷灵/客户端 UI" | 这些已实装 | 删除过时部分 |
| schema sample 旧境界字符串 | 若有 | 正典名 | grep + 替换 |

---

## §2 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 公式重写 + XP 曲线重算 + 50+ 测试更新 | 所有 cultivation 测试绿 + Realm 跃迁时长在 ±10% 区间 |
| **P1** ⬜ | doc comment / lingtian 头注释 / schema sample 文案统一 | grep 不到旧境界字符串 |
| **P2** ⬜ | client UI 文案 + 经脉图渲染调整(12+8=20 条) | client 显示正典名 + 经脉图正确 |

---

## §3 数据契约

- [ ] `server/src/cultivation/components.rs::required_meridians()` 公式重写
- [ ] `server/src/cultivation/breakthrough.rs::check_meridian_count` 同步
- [ ] `server/src/cultivation/xp_curve.rs` 重算斜率(满足 §三 时长基线)
- [ ] `agent/packages/schema/src/cultivation.ts` 校对常量
- [ ] `client/.../cultivation/CultivationScreen.java` UI 文案
- [ ] `client/.../cultivation/MeridianHud.java` 经脉图渲染调整

---

## §4 风险

- **临界路径起点** — 卡 1 周下游全停。建议**双人结对**或同时起 2 路并行实验
- XP 曲线数值需 telemetry 回填,初版按公式估算
- 50+ 测试同时更新可能漏几个 — 必须 `cargo test --all` 全绿才合并

---

## §5 开放问题

- [ ] 奇经 4 通(固元→通灵 必需) 是哪 4 条? worldview 未明确,需补 `cultivation-0006` 馆藏书细节
- [ ] XP 曲线是否同时支持 §F 三栏占比(修炼 50% 等)? 需调整 idle XP 自然回升
- [ ] `required_meridians()` 改动是否需要 server schema version bump? 影响存档兼容?

## §6 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §Q Wave 0 硬阻塞,所有下游依赖。
