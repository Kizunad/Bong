/**
 * plan-meridian-severed-v1 §1 P3 — 经脉永久 SEVERED 事件叙事渲染。
 *
 * 7 类来源（VoluntarySever / BackfireOverload / OverloadTear / CombatWound /
 * TribulationFail / DuguDistortion / Other）订阅 `bong:meridian_severed` 通道，
 * 按来源风格渲染叙事 → 发布到 AGENT_NARRATE。古意 / 末法残土风格——禁现代俚语 +
 * 禁生理标签（worldview §四:286）。
 *
 * 本模块只提供纯渲染函数 + 7 来源风格映射。Runtime 类（订阅 + 发布）由
 * plan-yidao-v1 / 各 v2 流派 plan 接入时再扩，避免本 plan 单独建 Redis runtime
 * 但下游零调用方。
 */

import type { Narration, MeridianId, MeridianSeveredEventV1, SeveredSource } from "@bong/schema";

/** 经脉中文名对照（用于叙事文本，避免英文 enum 出现在 narration 里）。 */
const MERIDIAN_NAMES: Record<MeridianId, string> = {
  Lung: "手太阴肺经",
  LargeIntestine: "手阳明大肠经",
  Stomach: "足阳明胃经",
  Spleen: "足太阴脾经",
  Heart: "手少阴心经",
  SmallIntestine: "手太阳小肠经",
  Bladder: "足太阳膀胱经",
  Kidney: "足少阴肾经",
  Pericardium: "手厥阴心包经",
  TripleEnergizer: "手少阳三焦经",
  Gallbladder: "足少阳胆经",
  Liver: "足厥阴肝经",
  Ren: "任脉",
  Du: "督脉",
  Chong: "冲脉",
  Dai: "带脉",
  YinQiao: "阴跷脉",
  YangQiao: "阳跷脉",
  YinWei: "阴维脉",
  YangWei: "阳维脉",
};

export function meridianName(id: MeridianId): string {
  return MERIDIAN_NAMES[id];
}

/** SeveredSource → 叙事文本片段。input/output 都是纯字符串拼接，无副作用，便于测试。 */
export function renderSeveredText(meridian: MeridianId, source: SeveredSource): string {
  const m = meridianName(meridian);
  if (typeof source === "object") {
    // Other
    const reason = source.Other;
    return `${m} 寂然而断，缘由不甚分明，旁人只见一道气息从此再无回响——${reason}。`;
  }
  switch (source) {
    case "VoluntarySever":
      // zhenmai ⑤ 主动断脉：自损以斩敌
      return `${m} 自截而断，断处齐整如刀过。这是有意为之的舍弃——以一脉之废，换一线生机。`;
    case "BackfireOverload":
      // 反噬累积：流派内功反扑
      return `${m} 在反噬之下久经撕扯，终于撑不住了。脉壁裂开一道贯口，真元逆流冲涌，自此成废。`;
    case "OverloadTear":
      // 强行调动超流量
      return `${m} 被一口压入数倍真元，脉壁应声而裂，再合不拢。强行调动者，便要担这个代价。`;
    case "CombatWound":
      // 战场被打到 SEVERED
      return `${m} 在搏杀里被一击穿透，末了那一寸再无气血温润。受这一伤的人，从此少了一寸江湖。`;
    case "TribulationFail":
      // 渡劫失败爆脉
      return `${m} 在天劫余威下崩裂，灵气倒灌入体，终是没能撑过最后一关。降境之苦，断脉是其中最重的一门。`;
    case "DuguDistortion":
      // 阴诡色侵蚀
      return `${m} 被阴诡色慢慢蚀透，脉络色泽乌黑，呼吸之间已无旧日真元。习毒道者，自身先化为药引。`;
  }
}

/** event → Narration（plan §6 narration runtime 入口）。返回 null 表示丢弃（无效 event）。 */
export function renderMeridianSeveredNarration(event: MeridianSeveredEventV1): Narration | null {
  if (event.type !== "meridian_severed") return null;
  const text = renderSeveredText(event.meridian_id, event.source);
  return {
    scope: "player",
    target: `meridian_severed:${event.entity_id}|${event.meridian_id}|tick:${event.at_tick}`,
    text,
    style: "narration",
  };
}
