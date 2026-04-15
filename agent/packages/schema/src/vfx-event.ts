import { Type, type Static } from "@sinclair/typebox";

/**
 * VFX 事件通道 `bong:vfx_event` S2C payload。
 *
 * 设计参考：`docs/plans-skeleton/plan-player-animation-v1.md §4.1` + `plan-particle-system-v1.md §2.2`。
 *
 * Phase 1 仅承载动画触发：
 *   - `play_anim`：服务端广播一次性动作（挥剑、突破、渡劫…）
 *   - `stop_anim`：终止某条持续动画（通常由状态切换驱动）
 *
 * 粒子类 VFX（sword_qi_slash、rune_draw 尾迹等）后续以新 type 加进 VfxEventV1 Union。
 *
 * 形态约束：
 *   - `target_player`：目标玩家 UUID（RFC 4122 canonical `8-4-4-4-12` 36 字符）
 *   - `anim_id`：`namespace:path` MC Identifier，小写 + 数字 + 下划线；注册表 lookup 时做 fallback
 *   - `priority`：`[100, 3999]` 覆盖 PlayerAnimator 分层区间（§3.3：100-499 姿态 / 500-999 移动 /
 *     1000-1999 战斗 / 2000-2999 受击 / 3000+ 剧情）。过低会被基础层盖住，过高留给 §4.4 动态注入
 *   - `fade_in_ticks` / `fade_out_ticks`：淡入淡出 tick 数，`[0, 40]`（20 tick = 1s，2s 上限对 VFX
 *     事件足够；更长的过渡走客户端自演状态机）
 *
 * 未纳入 Phase 1：
 *   - `speed` 字段 —— `KeyframeAnimationPlayer` 没现成 setSpeed API，v2 再扩
 *   - `play_anim_inline`（§4.4）—— 动态 JSON 注入路径，独立工单
 */

const UUID_PATTERN =
  "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$";

/** MC Identifier 形态：namespace:path，全小写、允许数字与下划线。 */
const ANIM_ID_PATTERN = "^[a-z0-9_]+:[a-z0-9_]+$";

/** 动画 priority 合法区间（对齐 PlayerAnimator 分层约定）。 */
export const VFX_ANIM_PRIORITY_MIN = 100;
export const VFX_ANIM_PRIORITY_MAX = 3999;

/** 淡入淡出 tick 上限（20 tick/s，即 2s）。 */
export const VFX_FADE_TICKS_MAX = 40;

export const VfxEventType = Type.Union([
  Type.Literal("play_anim"),
  Type.Literal("stop_anim"),
]);
export type VfxEventType = Static<typeof VfxEventType>;

export const VfxEventPlayAnimV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("play_anim"),
    target_player: Type.String({ pattern: UUID_PATTERN }),
    anim_id: Type.String({ pattern: ANIM_ID_PATTERN, maxLength: 128 }),
    priority: Type.Integer({
      minimum: VFX_ANIM_PRIORITY_MIN,
      maximum: VFX_ANIM_PRIORITY_MAX,
    }),
    fade_in_ticks: Type.Optional(
      Type.Integer({ minimum: 0, maximum: VFX_FADE_TICKS_MAX }),
    ),
  },
  { additionalProperties: false },
);
export type VfxEventPlayAnimV1 = Static<typeof VfxEventPlayAnimV1>;

export const VfxEventStopAnimV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("stop_anim"),
    target_player: Type.String({ pattern: UUID_PATTERN }),
    anim_id: Type.String({ pattern: ANIM_ID_PATTERN, maxLength: 128 }),
    fade_out_ticks: Type.Optional(
      Type.Integer({ minimum: 0, maximum: VFX_FADE_TICKS_MAX }),
    ),
  },
  { additionalProperties: false },
);
export type VfxEventStopAnimV1 = Static<typeof VfxEventStopAnimV1>;

export const VfxEventV1 = Type.Union([VfxEventPlayAnimV1, VfxEventStopAnimV1]);
export type VfxEventV1 = Static<typeof VfxEventV1>;
