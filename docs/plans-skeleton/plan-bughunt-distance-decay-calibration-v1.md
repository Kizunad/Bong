# plan-bughunt-distance-decay-calibration-v1（骨架）

> 一句话主题：按 worldview 与暗器 plan 的双锚点重校统一 `qi_physics` 距离衰减，使普通真元 10 格保留 40%、异变兽骨+凝实色 50 格约保留 80%，并核对真实 production 调用方。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 反解并拍板统一衰减模型/参数、纯计算语义与 R5 接缝 | ⬜ |
| P1 | R5-owned `qi_physics::{constants,distance}` 实现 + focused 调用方消费 | ⬜ |
| P2 | 双锚点/单调性/非法输入矩阵 + server gate | ⬜ |
| P3 | bot 远程伤害校准回归 | ⬜ |

## 接入面

- **进料**：`server/src/qi_physics/distance.rs:1-27` 的 `qi_distance_atten`、`MediumKind` color/carrier loss、`server/src/combat/decay.rs:8-24` 的 `hit_qi_ratio`、`server/src/combat/carrier.rs:1144-1145,1260-1324` 与 `server/src/qi_physics/collision.rs:70-147`。
- **出料**：命中/碰撞可用真元的 gameplay 数值；真实 production consumer 共享 canonical pure helper。
- **共享类型 / event**：复用 `MediumKind`、`CarrierGrade`、`ColorKind`；禁止另写 combat 私有 decay。
- **跨仓库契约**：无 payload 形状变化；玩家可感知数值必须补 bot/playtest。
- **worldview 锚点**：`worldview.md §四 L332-L340`（0 格 100%、普通 10 格 40%）与 §五 L405-L413（异变兽骨 50 格约 80%）；`docs/finished_plans/plan-anqi-v1.md` Q41 同锚。
- **qi_physics 锚点**：R5 是 `server/src/qi_physics/**` 唯一文件 owner；常数/公式由 R5 落地，focused plan 只拥有校准规格、combat integration 与 bot 验收。

## 当前证据（origin/main @ c625d5a5）

- `server/src/qi_physics/constants.rs:3-4` 仍是 `QI_DECAY_PER_BLOCK = 0.03`；`server/src/qi_physics/distance.rs:1-27` 使用指数 `(1-loss)^distance` 的纯函数。
- `server/src/combat/decay.rs:8-24` 把 `CarrierGrade::Beast` 映射为 `MediumKind::SpiritWeapon`；`:40-79` 的 regression test 锁 Mellow+BareQi@10 约 `0.737` 与 Solid+AncientRelic@50 约 `0.494`，后者并非 Beast 锚点。按当前 bonus，Solid+SpiritWeapon@50 约 `0.364`。
- `server/src/combat/needle.rs:95-223,292-392` 当前不调用 `hit_qi_ratio`、`qi_distance_atten` 或环境版 helper；needle 若未来接入属于新增 gameplay integration，不得伪称既有调用方迁移。

## P0 设计门

必须联合反解 base、color、carrier 三部分；不得只把 `QI_DECAY_PER_BLOCK` 改成一个能命中 10 格、却破坏 50 格 carrier 锚点的字面值。两个 pin 按当前类型映射写为 `Mellow + BareQi @10 ≈ 0.40` 与 `Solid + SpiritWeapon @50 ≈ 0.80`（`SpiritWeapon` 是 `CarrierGrade::Beast` 的当前映射）。若指数模型无法同时满足两个锚点，应在 P0 明确改模型并列全调用方影响。

## 纯计算与守恒边界（P0 决议）

- `qi_distance_atten` 保持纯数值函数：不写 `WorldQiAccount`、不创建 `QiTransfer`，也不为 `initial - arrived` 虚构 loss account；当前参数不足以识别真实 source、sink、命中与生命周期归属。此边界沿用 `docs/finished_plans/plan-qi-physics-v1.md:187-194`。
- 当前 production consumer 是 `combat::decay::hit_qi_ratio` 及其 carrier 路径、以及 `qi_physics::collision` 的环境版组合；`needle` 不是当前 caller。
- 若后续把 projectile `qi_payload` 定义为真实守恒余额，必须另由 R5 建立发射→飞行→命中/落空的唯一账户生命周期，明确 delivered qi 与未送达部分的 zone/overflow sink、transfer reason 和互斥 audit；本数值校准 plan 不旁路实现或宣称该账本生命周期已存在。

## 验收

1. 0 格=100%；Mellow+BareQi@10≈0.40；Solid+SpiritWeapon@50≈0.80（容差在 P0 冻结）。
2. 距离单调不增、普通载体不优于高级载体、非法/负/NaN 输入保持 `distance.rs:1-27` 既有契约。
3. `combat::decay`/carrier 与 `qi_physics::collision` 全部消费 canonical helper；static gate 禁止第二套 decay 常数/函数。不得把当前未接入的 needle 计为迁移完成。
4. R5 的 qi_physics unit pins 合入后，focused plan 运行完整 server gate，并以 bot 场景比较近距/10 格/50 格真实命中输出。

## 边界

- 不改技能基础伤害、瞄准、弹道速度或射程；只校准 gameplay 传输损耗。
- 本 plan 不直接修改 `server/src/qi_physics/**`；P0 可先完成反解/验收冻结，implementation 等 R5 常数/helper 合入后再消费，禁止与 R5 并行写文件。
- 不把候选但非 `origin/main` 祖先的历史提交当作已修证据。
