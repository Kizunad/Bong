# Bong · plan-gameplay-acceptance-v1 · 骨架

终极验收:6 段 E2E 脚本 + 100h 真实玩家实测。是 plan-gameplay-journey-v1 §H 的 plan 化交付。**所有前置 42 plan 完成后才能跑**——本 plan 是 Wave 5 收口的唯一 plan。

**世界观锚点**：`worldview.md` 全卷(本 plan 验证全部世界观一致性)

**交叉引用**：**全部 1-42 子 plan**(详 plan-gameplay-journey-v1 §Q.1) · `plan-gameplay-journey-v1` §H

---

## 接入面 Checklist

- **进料**：全部前置 plan 完成的 server / agent / client 二进制 + worldgen + library-web
- **出料**：6 段 E2E 脚本全绿 + 100h 实测视频 + 集成验收清单全勾
- **共享类型**：无新增,只调用既有 schema/IPC
- **跨仓库契约**：全栈
- **worldview 锚点**：全卷

---

## §0 设计轴心

- [ ] **不允许提前跑**：任何前置 plan 未完成不允许启动本 plan
- [ ] **6 段 + 100h 双重验收**：E2E 脚本验功能(自动化), 100h 实测验体验(真人)
- [ ] **真人 100h 实测无法压缩**：必须真玩家走通 — 这是 worldview 设定的物理时间
- [ ] **失败回滚**：任一段 fail → 不允许 finish,先修对应子 plan

---

## §1 6 段 E2E 脚本

```bash
scripts/e2e/p0-awaken.sh        # 0.5h 模拟,自动突破到引气
scripts/e2e/p1-induce.sh        # 3h 模拟,自动炼器+炼丹+灵田+突破到凝脉
scripts/e2e/p2-condense.sh      # 8h 模拟,流派选择+灵眼+第一次搜打撤
scripts/e2e/p3-solidify.sh      # 15h 模拟,布阵+副本周目+灵龛
scripts/e2e/p4-spirit.sh        # 25h 模拟,深副本+守家+大建造
scripts/e2e/p5-void.sh          # 渡虚劫三终局(succeed / intercepted / burst)
```

模拟模式可加速时间(物理 51.5h 跑实际 30 分钟内),但**真实路径必须 100h**。

---

## §2 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 6 段 E2E 脚本框架 + headless harness(无 client UI) | 脚本可启动 server+agent 跑模拟 |
| **P1** ⬜ | 每段脚本细节实装(指令序列 + 时间窗口) | 6 段单独跑全绿 |
| **P2** ⬜ | 集成验收清单(详 §3) | 所有勾选项验完 |
| **P3** ⬜ | 100h 真实玩家实测(招募 + 录像 + 反馈) | 1 名真玩家走通 P0-P5 |

---

## §3 集成验收清单(plan-gameplay-journey-v1 §H 移植)

- [ ] 6 段脚本全跑通且 server/agent/client 全程无 error log
- [ ] 每段产生的玩家状态可序列化保存(plan-persistence-v1)
- [ ] 每段产生的 narration 可读、有古意(narration-eval ✅ 验证 100% 通过)
- [ ] 每段流派分化后 SkillBar/QuickBar 渲染正确
- [ ] P5 渡劫期间 3 客户端联机,全部收到全服 narration
- [ ] 100h 实测:1 名真实玩家从 P0 走到 P5(成或失败) 一次
- [ ] 流派克制 telemetry 校准(plan-style-balance-v1 P3) 完成
- [ ] 化虚名额机制(plan-void-quota-v1) 验证: 多玩家同时起劫的判定正确
- [ ] 多周目第二世/第 n 世可达化虚(plan-multi-life-v1)

---

## §4 数据契约

- [ ] `scripts/e2e/p0-p5-*.sh` 6 段脚本
- [ ] `scripts/e2e/lib/harness.py` 共享 harness(client mock + server start + redis 监听)
- [ ] `scripts/e2e/lib/scenario.py` 各段场景 DSL
- [ ] `scripts/e2e/results/` 产出报告 + 截图
- [ ] `scripts/balance/100h_telemetry.py` 100h 实测数据聚合

---

## §5 实测协议

```
真玩家实测(P3):
  招募: 1 名熟悉 MC 但不参与 Bong 开发的玩家(签 NDA)
  时长: 100h(可分 4-6 周完成)
  记录: 全程屏幕录像 + 30 分钟一次自述音频 + 客观日志
  反馈: 每 P 段结束写 200 字总结
  失败标准:
    P0-P1 走不通 → 教学 plan(spawn-tutorial / poi-novice) 失败
    P2 走不通 → 流派 plan / spirit-eye 失败
    P3-P4 走不通 → 中阶 plan(niche / lingtian / botany / economy) 失败
    P5 走不通(渡劫死) → 接受(随机性 + 玩家技术),不算 plan 失败
  成功标准: P5 至少触发一次(渡劫成或失败均可)
```

---

## §6 退场判定

- 6 段 E2E 全绿
- 真玩家实测触发 P5(成或失败均算)
- 集成清单 9 项全勾
- 100h 通关数据回填给所有相关 plan(尤其 plan-style-balance-v1 校准)

完成后写 `## Finish Evidence` → `git mv` 入 `finished_plans/` → **整个 plan-gameplay-journey-v1 也可同步归档**。

---

## §7 开放问题

- [ ] 100h 实测玩家是否签 NDA(防泄露未发布内容)?
- [ ] P3 真玩家失败是 plan 设计问题还是个体问题(需多名玩家)? 决策倾向: 1 名 N=1 即可,失败深挖原因
- [ ] 6 段 E2E 脚本运行时长上限(模拟 51.5h 真实跑 30 分钟够不够)?
- [ ] 失败回滚:某段脚本 fail 后是否阻止本 plan finish? 决策: **是**,必须修对应子 plan
- [ ] 是否多客户端联机实测(2-3 玩家组队)? 决策倾向: 验完单玩家后再扩

## §8 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §H 派生。**所有前置完成后才能启动**(Wave 5 收口)。
