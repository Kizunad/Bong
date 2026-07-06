# plan-zhenfa-array-flag-e2e-wiring-v1（骨架）

> 主题：`array_flag`（`zhenfa.flag.basic` 配方正常产出，`category="tool"`）在 PR #962（plan-zhenfa-trap-client-equip-gate-v1）里已修好**装备门背离**（client 现在能把它装进手槽），但**装完仍功能惰性**——右键世界无任何反应、其唯一 server 作用也够不着。本 plan 把 `array_flag` 从"能装但没用"接到"能装且能用"的端到端可用状态。
>
> 来源：PR #962 的博弈 gate（opus 主审独立读码 + 变异级复核）发现，plan-zhenfa-trap-client-equip-gate-v1 §8.1 #2 point3 / P2 声称"四物品端到端可用 / 右键布置链路完整无缺口"对 `array_flag` **不成立**。故 #962 只修装备门、未归档母 plan。

## 阶段总览（骨架，待收口）

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 定 `array_flag` 的语义归属（右键布置 or 携带触发）并接通对应 client→server 链 | ⬜ | — |
| P1 | 视听 + HUD + narration（依 P0 决定的交互形态） | ⬜ | — |

## 已核实的缺口（PR #962 博弈证据，实施前需复核仍成立）

1. **右键布置无 case**：`client/src/main/java/com/bong/client/interaction/ClientInteractionItemResolver.java` 的 `zhenfaKindForItem()` switch 只映射 `warning_trap`/`blast_trap`/`slow_trap`/`beast_trap`/`trip_wire`/`bait_stake`/`gather_array_base`/`array_flag_basic`/`array_eye_basic`——**没有字面 `array_flag` 的 case**（`array_flag_basic` 是 `workbench_materials.toml` 里的另一个不同物品）。故装备 `array_flag` 后右键 → `zhenfaKindForItem` 返回 `null` → `MixinClientPlayerInteractionManagerAlchemy.bong$alchemyInteractBlock` 布置分支 skip → no-op。且 `ZhenfaKind` enum 无 `Flag` 变体、`ZhenfaLayoutScreen` 只 emit 6 种 ordinary-trap kind。
2. **carry-gate 客户端不可达**：`array_flag` 的唯一 server 作用是 `server/src/zhenfa/mod.rs` 的 `has_zhenfa_flag` 携带门（约 1626/1975/4085-4095，供 `handle_zhenfa_trigger_requests` 主动引爆用），但 client 的 `ClientRequestSender.sendZhenfaTrigger()`/`sendZhenfaDisarm()` 在 `client/src/main` 下**零调用**（仅测试引用）；Lingju 源 `gather_array_base` 是 `category="misc"` 装不上手。所以携带触发这条路玩家也够不着。

## 待收口的开放问题（§8 —— 立 active 前必须 pre-P0 收口）

1. `array_flag`（阵旗）在 worldview / zhenfa 系统里到底该是什么交互形态？
   - (a) 右键地面**布置**成一个阵法节点（类似三陷阱，但 array/网阵语义）→ 需给 `zhenfaKindForItem` 加 case + `ZhenfaKind` 加变体 + server 布置 handler + `ZhenfaLayoutScreen` 支持；
   - (b) 纯**携带触发**道具（持有即满足 `has_zhenfa_flag`，配合主动引爆/解除）→ 需把 `sendZhenfaTrigger`/`sendZhenfaDisarm` 接到某个 client 输入/UI；
   - (c) 两者都要。
   **这题涉及正典语义与玩法设计，必须人工拍板，不能 agent 自动定。**
2. 母 plan `docs/plan-zhenfa-trap-client-equip-gate-v1.md`（现仍 active）§8.1 #2 point3 / §P2 的"四物品端到端可用 / 链路完整无缺口"措辞对 `array_flag` 是**过度声称**——consume-plan 无权改写 §8.1，需**人工订正母 plan 该处措辞**（或在母 plan 归档时于 Finish Evidence 如实校正），再决定母 plan 归档时机。
3. `niche_house_puppet` / `niche_zhenfa_trap_{basic,middle,advanced}`（同 `category="tool"` 但零 craft/loot、仅 `/give`）是否借本 plan 一并考虑，还是继续排除（母 plan §8.1 #2 已排除）。

## 接入面（骨架初判，收口时补全）

- **进料**：`server/assets/items/zhenfa.toml` 的 `array_flag`（`category="tool"`）；`server/src/craft/mod.rs::register_zhenfa_v2_recipes` 的 `zhenfa.flag.basic` 配方。
- **出料**：依 §8 #1 决定——布置路走 `zhenfa_place` wire / `ZhenfaLayoutScreen`；携带触发路走 `sendZhenfaTrigger`/`has_zhenfa_flag`。
- **共享类型**：`ClientRequestProtocol.ZhenfaKind`（可能需加变体）/ `ClientInteractionItemResolver.zhenfaKindForItem` / server `handle_zhenfa_trigger_requests` / `has_zhenfa_flag`。
- **worldview 锚点**：阵法系统（§阵法/凡阶道具产出链路）——收口时 grep `docs/worldview.md` 确认阵旗/网阵的正典定位。
- **qi_physics 锚点**：若布置/触发涉及真元流转，必须走 `qi_physics::ledger`（收口时核）。

> 骨架状态，**未收口不得 consume**。§8 #1 是设计拍板题（人工）；§8 #2 是母 plan 文档订正（人工）。
