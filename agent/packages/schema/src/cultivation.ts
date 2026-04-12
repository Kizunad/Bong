/**
 * 修炼相关共享原子（plan-cultivation-v1 §6）。
 */
import { Type, type Static } from "@sinclair/typebox";

/** 6 境界常量（plan §1.1）。Rust Realm enum 映射。 */
export const Realm = Type.Union(
  [
    Type.Literal("Awaken"),
    Type.Literal("Induce"),
    Type.Literal("Condense"),
    Type.Literal("Solidify"),
    Type.Literal("Spirit"),
    Type.Literal("Void"),
  ],
  { description: "6 境界" },
);
export type Realm = Static<typeof Realm>;

/** 10 种染色（plan §1.1 ColorKind）。 */
export const ColorKind = Type.Union(
  [
    Type.Literal("Sharp"),
    Type.Literal("Heavy"),
    Type.Literal("Mellow"),
    Type.Literal("Solid"),
    Type.Literal("Light"),
    Type.Literal("Intricate"),
    Type.Literal("Gentle"),
    Type.Literal("Insidious"),
    Type.Literal("Violent"),
    Type.Literal("Turbid"),
  ],
  { description: "十种真元染色" },
);
export type ColorKind = Static<typeof ColorKind>;

/** 20 条经脉（12 正经 + 8 奇经）。 */
export const MeridianId = Type.Union(
  [
    Type.Literal("Lung"),
    Type.Literal("LargeIntestine"),
    Type.Literal("Stomach"),
    Type.Literal("Spleen"),
    Type.Literal("Heart"),
    Type.Literal("SmallIntestine"),
    Type.Literal("Bladder"),
    Type.Literal("Kidney"),
    Type.Literal("Pericardium"),
    Type.Literal("TripleEnergizer"),
    Type.Literal("Gallbladder"),
    Type.Literal("Liver"),
    Type.Literal("Ren"),
    Type.Literal("Du"),
    Type.Literal("Chong"),
    Type.Literal("Dai"),
    Type.Literal("YinQiao"),
    Type.Literal("YangQiao"),
    Type.Literal("YinWei"),
    Type.Literal("YangWei"),
  ],
  { description: "20 条经脉（12 正经 + 8 奇经）" },
);
export type MeridianId = Static<typeof MeridianId>;

/** 顿悟类别 A–G（plan §5.2）。 */
export const InsightCategory = Type.Union(
  [
    Type.Literal("Meridian"),
    Type.Literal("Qi"),
    Type.Literal("Composure"),
    Type.Literal("Coloring"),
    Type.Literal("Breakthrough"),
    Type.Literal("Style"),
    Type.Literal("Perception"),
  ],
  { description: "A–G 七类白名单" },
);
export type InsightCategory = Static<typeof InsightCategory>;

/** CultivationSnapshotV1 — plan §6.3，嵌入 WorldStateV1.players[].cultivation。 */
export const CultivationSnapshotV1 = Type.Object(
  {
    realm: Realm,
    qi_current: Type.Number({ minimum: 0 }),
    qi_max: Type.Number({ minimum: 0 }),
    qi_max_frozen: Type.Number({ minimum: 0 }),
    meridians_opened: Type.Integer({ minimum: 0, maximum: 20 }),
    meridians_total: Type.Integer({ minimum: 20, maximum: 20 }),
    qi_color_main: ColorKind,
    qi_color_secondary: Type.Optional(ColorKind),
    composure: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type CultivationSnapshotV1 = Static<typeof CultivationSnapshotV1>;

/** LifeRecordSnapshotV1 — plan §6.3，recent biography 摘要。 */
export const LifeRecordSnapshotV1 = Type.Object(
  {
    recent_biography_summary: Type.String({ maxLength: 4096 }),
  },
  { additionalProperties: false },
);
export type LifeRecordSnapshotV1 = Static<typeof LifeRecordSnapshotV1>;
