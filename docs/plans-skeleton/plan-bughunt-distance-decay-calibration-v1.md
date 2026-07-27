# plan-bughunt-distance-decay-calibration-v1（骨架）

> 一句话主题：按 worldview 与暗器 plan 的双锚点重校统一 `qi_physics` 距离衰减，使普通真元 10 格保留 40%、异变兽骨+凝实色 50 格约保留 80%，并核对全部调用方。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 反解并拍板统一衰减模型/参数 | ⬜ |
| P1 | `qi_physics::{constants,distance}` 单一实现与调用方迁移 | ⬜ |
| P2 | 双锚点/单调性/非法输入矩阵 + server gate | ⬜ |
| P3 | bot 远程伤害校准回归 | ⬜ |

## 接入面

- **进料**：`qi_physics::distance::qi_distance_atten`、`MediumKind` 的 color/carrier loss、combat/anqi/needle 调用方。
- **出料**：命中时到达目标的 qi；所有远程玩法共享一份公式。
- **共享类型 / event**：复用 `MediumKind`、`CarrierGrade`、`ColorKind`；禁止另写 combat 私有 decay。
- **跨仓库契约**：无 payload 形状变化；玩家可感知数值必须补 bot/playtest。
- **worldview 锚点**：`worldview.md §四 L332-L340`（0 格 100%、普通 10 格 40%）与 §五 L405-L413（异变兽骨 50 格约 80%）；`docs/finished_plans/plan-anqi-v1.md` Q41 同锚。
- **qi_physics 锚点**：所有新常数只落 `server/src/qi_physics/constants.rs`，公式只落 `distance.rs`。

## 当前证据（origin/main @ c625d5a5）

- `server/src/qi_physics/constants.rs:4` 仍是 `QI_DECAY_PER_BLOCK = 0.03`。
- `server/src/qi_physics/distance.rs` 使用指数 `(1-loss)^distance`。
- `server/src/combat/decay.rs:48,52` 的所谓 worldview test 仍锁普通 10 格 `0.737`、Relic+Solid 50 格 `0.494`，没有锁 Q41 的 `0.40/0.80`。

## P0 设计门

必须联合反解 base、color、carrier 三部分；不得只把 `QI_DECAY_PER_BLOCK` 改成一个能命中 10 格、却破坏 50 格 carrier 锚点的字面值。若指数模型无法同时满足两个锚点，应在 P0 明确改模型并列全调用方影响。

## 验收

1. 0 格=100%；Mellow+Mundane@10≈0.40；Solid+Beast@50≈0.80（容差在 P0 冻结）。
2. 距离单调不增、普通载体不优于高级载体、非法/负/NaN 输入按既有契约处理。
3. combat/anqi/needle 全部走 canonical helper；static gate 禁止第二套 decay 常数/函数。
4. server 完整 gate + bot 场景比较近距/10格/50格真实命中输出。

## 边界

- 不改技能基础伤害、瞄准、弹道速度或射程；只校准传输损耗。
- 不把候选但非 `origin/main` 祖先的历史提交当作已修证据。
