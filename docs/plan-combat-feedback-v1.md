# 战斗反馈闭环 V1 — 受伤感知 + 玩家攻击输入

> 补齐战斗系统的两个断链：**玩家被打没反馈** + **玩家攻击无效果**。
> 服务端战斗引擎（`plan-combat-no_ui.md`）和客户端 UI 骨架（`plan-combat-ui_impl.md`）已就绪，
> 但中间的**数据管线**断了——server 不发 `combat_event` payload 给 client，也不监听玩家攻击包。

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 受伤反馈管线（被打→看得见） | ⬜ |
| P1 | 玩家攻击输入（左键→打得到） | ⬜ |
| P2 | 噬元鼠咬人反馈（qi_damage 专用路径） | ⬜ |

---

## 接入面

- **进料**：
  - `combat::events::CombatEvent`（已有，resolve_attack_intents 产出）
  - `combat::rat_bite::RatBiteEvent`（已有，噬元鼠 AI 产出）
  - `valence::prelude::InteractEntityEvent`（已有，Valence 自动解析 MC 协议）
  - `combat::weapon::WeaponProfile`（已有，武器属性查表）
  - `player::PlayerInventory`（已有，手持物品查询）
- **出料**：
  - `bong:server_data` CustomPayload `"combat_event"` → client `CombatEventHandler` → `DamageFloaterStore`（已有 handler，缺 server 发送）
  - `bong:server_data` CustomPayload `"event_stream_push"` → 事件流文本（已接通）
  - 原版 MC `EntityStatusS2c`（红闪 hurt 动画）
  - `AudioEvent`（已有管线，缺 combat 音效触发）
- **共享类型 / event**：
  - 复用 `CombatEvent`、`AttackIntent`（不新建）
  - 复用 `combat_event` payload schema（client 已有 handler）
  - `InteractEntityEvent` 的 `EntityInteraction::Attack` variant（Valence 内建，当前被 inventory 忽略）
- **跨仓库契约**：
  - server → client：`bong:server_data` channel，type=`"combat_event"`，schema 见 `CombatEventHandler.java:15-26`
  - server → client：原版 MC `EntityStatusS2c`（entity_id + status=HURT）
  - client → server：原版 MC `InteractEntity` 包（Valence 自动解析为 `InteractEntityEvent`，无需自定义协议）
- **worldview 锚点**：§四 战斗系统（"被攻击同时掉血和掉真元"）、§四:248 伤口类型表

---

## P0 — 受伤反馈管线

> 目标：NPC 攻击玩家 → 玩家看到伤害飘字 + 红闪 + 事件流文本。

### P0.1 Server: `emit_combat_event_to_client` system

新增 `server/src/network/combat_event_emit.rs`：

- 消费 `EventReader<CombatEvent>`
- 对每个 event，组装 `combat_event` JSON payload：
  ```json
  {
    "events": [{
      "kind": "hit" | "crit" | "block" | "qi_damage",
      "amount": 12.5,
      "x": <target_pos.x>, "y": <target_pos.y>, "z": <target_pos.z>,
      "text": "12"
    }]
  }
  ```
- 通过 `bong:server_data` CustomPayload 发给 **target**（受击者看到飘字）
- 同时发给 **attacker**（攻击者看到命中确认）
- kind 映射：`WoundKind::*` → `"hit"` / `"crit"`（暴击判定后续接）；防御触发 → `"block"`
- 注册到 `CombatSystemSet::Emit` phase，在 `event_stream_emit` 之后

### P0.2 Server: 原版 hurt 动画包

在 `emit_combat_event_to_client` 同一 system 内：

- 对 target entity 写入 `valence::prelude::EntityStatuses`，设置 hurt status（MC status byte 2 = 受伤）
- 这让原版 MC 客户端播放：红闪叠加 + 受击倾斜 + "嗷" 音效
- 无需客户端改动，Valence 框架自动同步 EntityStatuses 到协议包

### P0.3 测试

- `combat_event_emit::emit_sends_payload_to_target` — CombatEvent 触发后，target 的 CustomPayload 队列有 `combat_event` payload
- `combat_event_emit::emit_sends_payload_to_attacker` — 攻击者也收到
- `combat_event_emit::entity_status_hurt_set_on_target` — target 的 EntityStatuses 包含 hurt
- `combat_event_emit::no_emit_for_zero_damage` — damage=0 不发（防 spam）
- `combat_event_emit::kind_maps_correctly` — WoundKind → wire kind 映射覆盖全部 variant

---

## P1 — 玩家攻击输入

> 目标：玩家左键实体 → 产生 AttackIntent → 走正常战斗解算 → 目标受伤。

### P1.1 Server: `handle_player_attack` system

新增 `server/src/combat/player_attack.rs`：

- 消费 `EventReader<InteractEntityEvent>`，过滤 `EntityInteraction::Attack`
- 校验攻击者是 `Client` entity（非 NPC）
- 查询攻击者手持物品 → `WeaponProfile` 查表（空手 = `FIST` profile，已有常量 `FIST_REACH`）
- 距离校验：`attacker_pos.distance(target_pos) <= weapon.reach + 0.5`（0.5 容差，MC 网络延迟）
- 攻击冷却：component `PlayerAttackCooldown { last_tick: u64 }`，间隔 < `ATTACK_COOLDOWN_TICKS`（10 ticks = 0.5s）则忽略
- 扣 stamina：`ATTACK_STAMINA_COST`（已有常量 = 3.0）
- stamina 不足 → 忽略攻击（不发 AttackIntent）
- 生成 `AttackIntent { attacker, target, qi_invest: 0.0, wound_kind: weapon.wound_kind, source: weapon.source, ... }`
- `qi_invest` 默认 0.0（普通攻击不注入真元；后续功法系统走 skill cast 路径）
- 注册到 `CombatSystemSet::Intent` phase

### P1.2 Server: 攻击反作弊

在 `player_attack.rs` 同一 system 内：

- 距离超限 → log warn + 忽略（不踢人，MC 延迟常见）
- 冷却内重复攻击 → 静默忽略
- 攻击自己 → 忽略
- 攻击已死亡 entity（`Lifecycle` 状态 Dead/NearDeath）→ 忽略

### P1.3 测试

- `player_attack::attack_generates_intent` — InteractEntityEvent(Attack) → AttackIntent 产出
- `player_attack::fist_uses_default_profile` — 空手攻击使用 FIST 参数
- `player_attack::weapon_modifies_damage` — 持武器时 AttackIntent.source 和 wound_kind 正确
- `player_attack::out_of_range_ignored` — 距离超限不产出 AttackIntent
- `player_attack::cooldown_prevents_spam` — 冷却内重复攻击被忽略
- `player_attack::stamina_insufficient_ignored` — stamina 不够不攻击
- `player_attack::dead_target_ignored` — 攻击死亡 entity 无效
- `player_attack::self_attack_ignored` — 攻击自己无效
- `player_attack::npc_interaction_not_hijacked` — NPC 的 InteractEntityEvent 不被此 system 拦截（只处理 Client entity）

---

## P2 — 噬元鼠咬人反馈

> 目标：噬元鼠咬玩家 → 灵气减少飘字 + 受击反馈 + 音效。
> 噬元鼠不走正常 CombatEvent（它吸灵气不造伤口），需要单独的反馈路径。

### P2.1 Server: `rat_bite.rs` 追加反馈

在 `apply_rat_bite_qi_drain` system 内，每次成功吸取 qi 后：

- 组装 `combat_event` payload：`kind="qi_damage"`, `amount=qi_drained`, `text="-{qi}"`, `color=0xFF80A0FF`（蓝色，与客户端 `qi_damage` 默认色一致）
- 通过 `bong:server_data` CustomPayload 发给被咬玩家
- 设置被咬者 `EntityStatuses` hurt（红闪）
- 发 `PlaySoundRecipeRequest`：recipe=`"rat_bite"`（新建 audio recipe：嘶嘶+啃咬混音）

### P2.2 Audio recipe

在 `server/src/network/audio_trigger.rs` 的 recipe 注册中追加：

- `"rat_bite"` → layers: `[{ sound: "entity.silverfish.hurt", pitch: 1.2, volume: 0.5 }, { sound: "entity.player.hurt", pitch: 0.9, volume: 0.3, delay_ticks: 1 }]`

### P2.3 测试

- `rat_bite_feedback::bite_emits_combat_event_payload` — 被咬后 CustomPayload 队列有 `combat_event`（kind=qi_damage）
- `rat_bite_feedback::bite_sets_hurt_status` — 被咬者 EntityStatuses 包含 hurt
- `rat_bite_feedback::bite_triggers_audio` — 被咬后有 `PlaySoundRecipeRequest`
- `rat_bite_feedback::zero_qi_drain_no_feedback` — qi 已经为 0 不再重复反馈

---

## 不做的事（边界）

- **不改 `ClientRequestV1` / `ClientPayloadV1`**：玩家攻击走原版 MC 协议（InteractEntity），无需自定义 C2S 包
- **不加暴击系统**：P0 的 `"crit"` kind 预留但不实装判定逻辑，后续由功法系统接入
- **不加击退/击飞**：需要 Valence velocity 包支持，另开 plan
- **不改 NPC 攻击路径**：NPC 已有 brain melee → AttackIntent 管线，不动
- **不加 PvP**：P1 的 `handle_player_attack` 只允许攻击 NPC entity（has `NpcMarker`），PvP 另议
- **不动 Redis 战斗通道**：`bong:combat_realtime` / `bong:combat_summary` 已工作，本 plan 不改

---

## 文件变更清单

| 文件 | 操作 | 阶段 |
|------|------|------|
| `server/src/network/combat_event_emit.rs` | **新建** — CombatEvent → client combat_event payload + EntityStatuses hurt | P0 |
| `server/src/network/mod.rs` | **改** — 注册 `emit_combat_event_to_client` system 到 Emit phase | P0 |
| `server/src/combat/player_attack.rs` | **新建** — InteractEntityEvent(Attack) → AttackIntent | P1 |
| `server/src/combat/mod.rs` | **改** — 注册 `handle_player_attack` system 到 Intent phase + `PlayerAttackCooldown` component | P1 |
| `server/src/combat/rat_bite.rs` | **改** — 追加 combat_event payload + EntityStatuses hurt + PlaySoundRecipeRequest | P2 |
| `server/src/network/audio_trigger.rs` | **改** — 追加 `"rat_bite"` recipe | P2 |
