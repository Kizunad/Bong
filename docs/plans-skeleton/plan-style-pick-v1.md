# Bong · plan-style-pick-v1 · 骨架

流派触发事件 + schema 登记。**不锁流派**(O.14),但每次玩家通过"染色丹/染色事件"激活新流派需登记到 `UnlockedStyles`,客户端解锁对应 SkillBar 槽位。

**世界观锚点**：`worldview.md §五.七流派` · `§六.三 个性三层(经脉/染色/顿悟)` · `§六 line 1`(路径倾向非职业锁)

**library 锚点**：`cultivation-0005 真元十一色考` · `peoples-0006 战斗流派源流`

**交叉引用**：`plan-cultivation-v1` ✅ · `plan-cultivation-canonical-align-v1` ⬜ · `plan-multi-style-v1` ⬜ · `plan-style-balance-v1` ⬜ · `plan-gameplay-journey-v1` §A/§A.5/O.14

---

## 接入面 Checklist

- **进料**：玩家拾取染色丹/触发染色事件/到达特定地形
- **出料**：`UnlockedStyles` 增量 + client SkillBar 槽位解锁视觉 + tiandao "道路确认" narration
- **共享类型**：`StyleId` enum(`Baomai | Anqi | Zhenfa | Dugu | Jiemai | Tuike | Woliu`) + `UnlockedStyles` ✅(已实装)
- **跨仓库契约**：新增 `client-request.style-unlock` schema + server 校验 + agent narration trigger
- **worldview 锚点**：§六 line 1 + §五

---

## §0 设计轴心

- [ ] **不锁流派**——玩家可不断解锁,直到 7 全开(全流派精通见 plan-multi-style-v1)
- [ ] **触发不可预测**(O.13)：环境线索/染色丹/特殊事件,**没有 UI 让玩家"选"**
- [ ] **解锁可读化**：解锁瞬间客户端给视觉反馈(SkillBar 槽位发光) + tiandao 一句台词

---

## §1 触发源（worldview 各流派的入门事件）

| 流派 | 触发线索 | 区域 |
|---|---|---|
| 体修(爆脉) | 找到一具断臂修士尸体 + "血肉之躯亦是法器"残卷 | broken_peaks |
| 器修(暗器) | 拾"含毒铜针残卷" 或 拥有 3+ 异变兽骨 | cave_network |
| 地师(阵法) | 目睹废弃欺天阵 + 拾"地师手记"残页 | spring_marsh |
| 毒蛊 | 蛛巢内取出第一只蛊母 | rift_valley |
| 截脉(震爆) | 凝脉突破事件后随机触发 + 玩家曾被高频脉冲攻击过 | 任意 |
| 替尸(蜕壳) | 找到上一任替尸者残蜕 | cave_network |
| 涡流(绝灵) | 负压区静修 30s+ | waste_plateau 边缘 |

---

## §2 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | StyleId enum + StyleUnlockEvent + server 校验 + UnlockedStyles 增量 | 单元测试覆盖 7 流派触发路径 |
| **P1** ⬜ | client SkillBar 槽位解锁视觉 + tiandao "道路确认" narration | 玩家解锁瞬间有反馈 |
| **P2** ⬜ | 7 流派触发 POI/事件接入 worldgen + 各 plan | 各流派触发线索可被玩家发现 |

---

## §3 数据契约

- [ ] `server/src/cultivation/style.rs` StyleId enum + UnlockedStyles 扩展
- [ ] `server/src/cultivation/style_unlock.rs` 触发逻辑 + 校验
- [ ] `agent/packages/schema/src/cultivation.ts` 增 `StyleUnlockEvent`
- [ ] `client/.../combat/UnlockedStylesStore.java` ✅ 已实装,扩展支持多解锁
- [ ] `client/.../hud/SkillBarUnlockEffect.java` 解锁视觉

---

## §4 开放问题

- [ ] 触发是 100% 还是概率(避免玩家网上抄作业一查就拿)?
- [ ] 触发后能否取消(后悔药)? 决策: **不能**,但可继续解锁其他流派(全精通)
- [ ] 多人同场触发(如同一蛊母)归属判定?
- [ ] 触发线索是否会被消耗(蛊母被取走后他人无法触发)?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §A / O.14 派生。
