# Bong · plan-void-quota-v1 · 骨架

化虚名额按**世界灵气总量**动态决定:`化虚名额上限 = floor(世界灵气总量 / K)`。超额玩家起劫 → 直接降"绝壁劫"(强度 ×1.5,无法过)。

**世界观锚点**：`worldview.md §三 line 145-150`(全服 1-2 人化虚,末法天道无力承担更多) · `§八 天道行为准则`(灵气总量调控)

**交叉引用**：`plan-tribulation-v1` ⏳(渡虚劫流程) · `plan-cultivation-canonical-align-v1` ⬜ · `plan-gameplay-journey-v1` §N.0/O.3

---

## 接入面 Checklist

- **进料**：`world::zone::ZoneRegistry` 全 zone 灵气浓度场 + 当前化虚者计数
- **出料**：化虚名额 quota 计算 + 超额时的绝壁劫降级
- **共享类型**：`VoidQuotaState` schema(`current_void_count` / `total_world_qi` / `quota_max` / `K_value`)
- **跨仓库契约**：server quota check + agent narration("绝壁劫"专属) + client inspect UI 显示当前名额
- **worldview 锚点**：§三 line 145-150 + §八

---

## §0 设计轴心

- [ ] **不固定名额**：按灵气总量动态算,玩家行为(灵田过度抽吸/坍缩渊塌缩域崩)影响名额
- [ ] **化虚者死亡名额回流**：让后人有路
- [ ] **K 值为 config**：开服初期 K 偏大(名额 1-2),后期可调
- [ ] **超额惩罚直接**：超额玩家试图渡虚劫 → 强度 ×1.5 必死,**不留侥幸**

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 公式实装: `quota = floor(total_qi / K)` + tribulation::check_void_quota | 公式可计算 + tribulation 校验 |
| **P1** ⬜ | 超额时降"绝壁劫"(tribulation 阶段强度 ×1.5) + tiandao 专属 narration | 超额玩家试图起劫时被打回(必死) |
| **P2** ⬜ | 化虚者死亡 → 灵气回流 + 名额可立刻再开 | death-lifecycle 死亡 hook 接入 |
| **P3** ⬜ | client inspect UI 显示当前世界化虚名额: X / Y | 通灵期玩家可知是否能起劫 |

---

## §2 关键公式

```
total_world_qi = sum(zone.qi_density × zone.area for zone in ZoneRegistry)
quota_max      = floor(total_world_qi / K)
can_void(player) = (current_void_count < quota_max)

绝壁劫触发(超额时玩家起劫):
  tribulation_strength = base_strength × 1.5
  narration: "天地装不下你了。"
  结果: 100% 渡劫失败(真死)
```

K 值校准（运维 config）:
```
开服初期(配 100 in-game year):
  K = 5000
  total_qi ~ 10000 → quota_max = 2

长期(K 不变,但灵田/坍缩渊塌缩 → 灵气下降):
  total_qi ~ 5000 → quota_max = 1
  total_qi < 5000 → quota_max = 0(无人能化虚,直到回升或化虚者死)
```

---

## §3 数据契约

- [ ] `server/src/cultivation/tribulation/void_quota.rs` 公式 + check
- [ ] `server/src/world/total_qi.rs` 全 zone 灵气总和(每 1 in-game day 重算缓存)
- [ ] `server/src/cultivation/tribulation/jueb_strike.rs` 绝壁劫降级
- [ ] `agent/.../skills/calamity.md` "天地装不下你了" narration
- [ ] `client/.../inspect/VoidQuotaDisplay.java` 名额 UI(通灵期可见)

---

## §4 开放问题

- [ ] K 值如何动态调整(运维 config 还是基于 telemetry)? 决策倾向: 初版固定,后续 telemetry
- [ ] "灵气总量" 是按 zone 浓度积分还是按某种加权(中心 zone vs 边缘)?
- [ ] 化虚者死亡灵气回流的计算(死了就 +K? 还是按其化虚池剩余?)
- [ ] 名额 0 时通灵满玩家如何引导(narration 暗示等前辈死)?
- [ ] 多名通灵玩家同时起劫(同一 tick 触发) → 谁先获得名额? FCFS 还是 random?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §N.0 / O.3 派生。
