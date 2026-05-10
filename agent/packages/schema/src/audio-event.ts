import { Type, type Static } from "@sinclair/typebox";

/** S2C audio channels: `bong:audio/play` and `bong:audio/stop`. */

const RECIPE_ID_PATTERN = "^[a-z0-9_]+$";
const IDENTIFIER_PATTERN = "^[a-z0-9_.-]+:[a-z0-9_./-]+$";

export const AUDIO_VOLUME_MAX = 4;
export const AUDIO_PITCH_MIN = 0.1;
export const AUDIO_PITCH_MAX = 2;
export const AUDIO_PRIORITY_MAX = 100;

export const AudioAttenuationV1 = Type.Union([
  Type.Literal("player_local"),
  Type.Literal("world_3d"),
  Type.Literal("global_hint"),
  Type.Literal("zone_broadcast"),
]);
export type AudioAttenuationV1 = Static<typeof AudioAttenuationV1>;

export const AudioSoundCategoryV1 = Type.Union([
  Type.Literal("MASTER"),
  Type.Literal("HOSTILE"),
  Type.Literal("AMBIENT"),
  Type.Literal("VOICE"),
  Type.Literal("BLOCKS"),
]);
export type AudioSoundCategoryV1 = Static<typeof AudioSoundCategoryV1>;

export const SoundLayerV1 = Type.Object(
  {
    sound: Type.String({ pattern: IDENTIFIER_PATTERN, maxLength: 128 }),
    volume: Type.Number({ minimum: 0, maximum: AUDIO_VOLUME_MAX }),
    pitch: Type.Number({ minimum: AUDIO_PITCH_MIN, maximum: AUDIO_PITCH_MAX }),
    delay_ticks: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SoundLayerV1 = Static<typeof SoundLayerV1>;

export const LoopConfigV1 = Type.Object(
  {
    interval_ticks: Type.Integer({ minimum: 1 }),
    while_flag: Type.String({ minLength: 1, maxLength: 64 }),
  },
  { additionalProperties: false },
);
export type LoopConfigV1 = Static<typeof LoopConfigV1>;

export const SoundRecipeV1 = Type.Object(
  {
    id: Type.String({ pattern: RECIPE_ID_PATTERN, maxLength: 128 }),
    layers: Type.Array(SoundLayerV1, { minItems: 1, maxItems: 8 }),
    loop: Type.Optional(LoopConfigV1),
    priority: Type.Integer({ minimum: 0, maximum: AUDIO_PRIORITY_MAX }),
    attenuation: AudioAttenuationV1,
    category: AudioSoundCategoryV1,
  },
  { additionalProperties: false },
);
export type SoundRecipeV1 = Static<typeof SoundRecipeV1>;

export const PlaySoundRecipeEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    recipe_id: Type.String({ pattern: RECIPE_ID_PATTERN, maxLength: 128 }),
    instance_id: Type.Integer({ minimum: 0 }),
    pos: Type.Optional(Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()])),
    flag: Type.Optional(Type.String({ minLength: 1, maxLength: 64 })),
    volume_mul: Type.Number({ minimum: 0, maximum: AUDIO_VOLUME_MAX }),
    pitch_shift: Type.Number({ minimum: -1, maximum: 1 }),
    recipe: SoundRecipeV1,
  },
  { additionalProperties: false },
);
export type PlaySoundRecipeEventV1 = Static<typeof PlaySoundRecipeEventV1>;

export const StopSoundRecipeEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    instance_id: Type.Integer({ minimum: 1 }),
    fade_out_ticks: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type StopSoundRecipeEventV1 = Static<typeof StopSoundRecipeEventV1>;

export const AmbientZoneEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    zone_name: Type.String({ minLength: 1, maxLength: 128 }),
    ambient_recipe_id: Type.String({ pattern: RECIPE_ID_PATTERN, maxLength: 128 }),
    music_state: Type.Union([
      Type.Literal("AMBIENT"),
      Type.Literal("COMBAT"),
      Type.Literal("CULTIVATION"),
      Type.Literal("TSY"),
      Type.Literal("TRIBULATION"),
    ]),
    is_night: Type.Boolean(),
    season: Type.String({ minLength: 1, maxLength: 32 }),
    tsy_depth: Type.Optional(Type.Union([
      Type.Literal("shallow"),
      Type.Literal("mid"),
      Type.Literal("deep"),
    ])),
    fade_ticks: Type.Integer({ minimum: 0 }),
    pos: Type.Optional(Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()])),
    volume_mul: Type.Number({ minimum: 0, maximum: AUDIO_VOLUME_MAX }),
    pitch_shift: Type.Number({ minimum: -1, maximum: 1 }),
    recipe: SoundRecipeV1,
  },
  { additionalProperties: false },
);
export type AmbientZoneEventV1 = Static<typeof AmbientZoneEventV1>;

export const AudioEventV1 = Type.Union([
  PlaySoundRecipeEventV1,
  StopSoundRecipeEventV1,
  AmbientZoneEventV1,
]);
export type AudioEventV1 = Static<typeof AudioEventV1>;
