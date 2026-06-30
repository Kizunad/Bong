import { Type, type Static } from "@sinclair/typebox";

/**
 * VFX 事件通道 `bong:vfx_event` S2C payload。
 *
 * 设计参考：`docs/plans-skeleton/plan-player-animation-v1.md §4.1` + `plan-particle-system-v1.md §2.2`。
 *
 * 当前支持 5 个 variant：
 *   - `play_anim`：服务端广播一次性动作（挥剑、突破、渡劫…），按玩家 UUID 寻人
 *   - `stop_anim`：终止某条持续动画（通常由状态切换驱动）
 *   - `spawn_particle`：触发一次自定义粒子（Bong 独有：剑气 / 符文 / 灵压涟漪…）
 *   - `play_anim_inline`：天道 Agent / dev LLM 直接注入完整 Emotecraft v3 JSON 并播放
 *   - `play_entity_anim`：按 MC 协议 entity_id 触发**非玩家实体**（GeckoLib FaunaEntity）的
 *     一次性招式动画。黑武士这类无 UUID 的 Marker boss 走此路径（UUID 寻人走不通）
 *
 * 形态约束：
 *   - `target_player`：目标玩家 UUID（RFC 4122 canonical `8-4-4-4-12` 36 字符）
 *   - `anim_id` / `event_id`：`namespace:path` MC Identifier 形态，小写+数字+下划线
 *   - `priority`：`[100, 3999]` 覆盖 PlayerAnimator 分层区间（§3.3：100-499 姿态 / 500-999 移动 /
 *     1000-1999 战斗 / 2000-2999 受击 / 3000+ 剧情）。过低会被基础层盖住，过高留给 §4.4 动态注入
 *   - `fade_in_ticks` / `fade_out_ticks`：淡入淡出 tick 数，`[0, 40]`（20 tick = 1s，2s 上限对 VFX
 *     事件足够；更长的过渡走客户端自演状态机）
 *   - 粒子 `origin` / `direction`：各 3 个 finite number（见 `plan-particle-system-v1 §2.2`）
 *   - 粒子 `color`：CSS 形态 `#RRGGBB` hex（客户端解析为 0xRRGGBB）
 *   - 粒子 `strength`：`[0.0, 1.0]` 归一化强度
 *   - 粒子 `count`：`[1, 64]` 同 tick 合批
 *   - 粒子 `duration_ticks`：`[1, 200]`（20 tick = 1s，上限 10s 够一次性事件）
 *
 * 未纳入当前版本：
 *   - `speed` 字段 —— `KeyframeAnimationPlayer` 没现成 setSpeed API，v2 再扩
 */

const UUID_PATTERN =
  "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$";

/** MC Identifier 形态：namespace:path，全小写、允许数字与下划线。粒子 event_id 复用同一正则。 */
const ANIM_ID_PATTERN = "^[a-z0-9_]+:[a-z0-9_]+$";

/** CSS 形态 `#RRGGBB`（6 位 hex，小写）。Phase 1 不支持 alpha 通道（`#RRGGBBAA`）。 */
const COLOR_HEX_PATTERN = "^#[0-9a-fA-F]{6}$";

/** 动画 priority 合法区间（对齐 PlayerAnimator 分层约定）。 */
export const VFX_ANIM_PRIORITY_MIN = 100;
export const VFX_ANIM_PRIORITY_MAX = 3999;

/** 淡入淡出 tick 上限（20 tick/s，即 2s）。 */
export const VFX_FADE_TICKS_MAX = 40;

/** 粒子同 tick 合批上限（plan §2.5：超出按优先级丢弃由服务端处理）。 */
export const VFX_PARTICLE_COUNT_MAX = 64;

/** 粒子持续时间上限（tick）。20 tick/s → 10s，一次性事件够用。 */
export const VFX_PARTICLE_DURATION_TICKS_MAX = 200;

/**
 * 实体一次性动画持续上限（tick）。20 tick/s → 10s。客户端到时回 idle loop。
 * 与粒子上限同值但语义独立（一个管粒子寿命，一个管动画占用时长），故单列常量。
 */
export const VFX_ENTITY_ANIM_DURATION_TICKS_MAX = 200;

/** 实体动画名最大长度（GeckoLib animation 名，如 animation.bong.heiwushi.dark_barrage）。 */
export const VFX_ENTITY_ANIM_NAME_MAX_CHARS = 128;

/** inline 动画 JSON 字符串上限。最终 CustomPayload 仍受 MAX_PAYLOAD_BYTES 兜底。 */
export const VFX_INLINE_ANIM_JSON_MAX_CHARS = 4096;

export const VfxEventType = Type.Union([
  Type.Literal("play_anim"),
  Type.Literal("stop_anim"),
  Type.Literal("spawn_particle"),
  Type.Literal("play_anim_inline"),
  Type.Literal("play_entity_anim"),
]);
export type VfxEventType = Static<typeof VfxEventType>;

/** 3D 向量（origin / direction）。Float32 精度在客户端表演足够，TypeBox 不限幅。 */
const Vec3 = Type.Tuple([Type.Number(), Type.Number(), Type.Number()]);

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

export const VfxEventPlayAnimInlineV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("play_anim_inline"),
    target_player: Type.String({ pattern: UUID_PATTERN }),
    anim_id: Type.String({ pattern: ANIM_ID_PATTERN, maxLength: 128 }),
    anim_json: Type.String({ minLength: 1, maxLength: VFX_INLINE_ANIM_JSON_MAX_CHARS }),
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
export type VfxEventPlayAnimInlineV1 = Static<typeof VfxEventPlayAnimInlineV1>;

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

/**
 * 粒子触发 variant（plan-particle-system-v1 §2.2 + §5.1）。
 *
 * 客户端按 `event_id` 查 `VfxRegistry`（plan §2.7），分发到对应 `VfxPlayer`。
 * `origin`/`direction`/`color`/`strength` 是共同参数，每个 player 自己决定用哪几个。
 * `count` 为同 tick 合批，客户端一次触发会生成多个粒子（随机抖动由客户端自由）。
 */
export const VfxEventSpawnParticleV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("spawn_particle"),
    event_id: Type.String({ pattern: ANIM_ID_PATTERN, maxLength: 128 }),
    origin: Vec3,
    direction: Type.Optional(Vec3),
    color: Type.Optional(Type.String({ pattern: COLOR_HEX_PATTERN })),
    strength: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    count: Type.Optional(
      Type.Integer({ minimum: 1, maximum: VFX_PARTICLE_COUNT_MAX }),
    ),
    duration_ticks: Type.Optional(
      Type.Integer({ minimum: 1, maximum: VFX_PARTICLE_DURATION_TICKS_MAX }),
    ),
  },
  { additionalProperties: false },
);
export type VfxEventSpawnParticleV1 = Static<typeof VfxEventSpawnParticleV1>;

/**
 * 实体一次性动画触发（黑武士 boss 出招）。
 *
 * <p>不同于 {@link VfxEventPlayAnimV1} 按玩家 UUID 寻人：黑武士是无 UUID 的 Marker 实体，
 * 客户端按 MC 协议 {@code entity_id}（{@code world.getEntityById}）定位 {@code FaunaEntity}，
 * 调 {@code triggerAction(anim, duration_ticks)} 播一次性招式动画，{@code duration_ticks} 到时回 idle。
 *
 * @property entity_id      MC 协议 entity_id（Valence {@code EntityId::get()}，i32，≥1）
 * @property anim           GeckoLib 动画名（free-form，如 {@code animation.bong.heiwushi.dark_barrage}）
 * @property duration_ticks 动画占用时长 [1, {@link VFX_ENTITY_ANIM_DURATION_TICKS_MAX}]
 */
export const VfxEventPlayEntityAnimV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("play_entity_anim"),
    entity_id: Type.Integer({ minimum: 1 }),
    anim: Type.String({ minLength: 1, maxLength: VFX_ENTITY_ANIM_NAME_MAX_CHARS }),
    duration_ticks: Type.Integer({
      minimum: 1,
      maximum: VFX_ENTITY_ANIM_DURATION_TICKS_MAX,
    }),
  },
  { additionalProperties: false },
);
export type VfxEventPlayEntityAnimV1 = Static<typeof VfxEventPlayEntityAnimV1>;

export const VfxEventV1 = Type.Union([
  VfxEventPlayAnimV1,
  VfxEventPlayAnimInlineV1,
  VfxEventStopAnimV1,
  VfxEventSpawnParticleV1,
  VfxEventPlayEntityAnimV1,
]);
export type VfxEventV1 = Static<typeof VfxEventV1>;
