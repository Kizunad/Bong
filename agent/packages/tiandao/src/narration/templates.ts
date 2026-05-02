import type { Narration, PseudoVeinDissipateEventV1, PseudoVeinSnapshotV1 } from "@bong/schema";

export type PseudoVeinNarrationKey =
  | "pseudo_vein.lure"
  | "pseudo_vein.warning"
  | "pseudo_vein.dissipate";

export const PSEUDO_VEIN_NARRATION_TEMPLATES: Record<PseudoVeinNarrationKey, string> = {
  "pseudo_vein.lure":
    "荒野深处忽有花气浮起，灵光亮得太整齐，像有人把机缘端在明处。此兆将引众修近前，先闻香者未必先得道。",
  "pseudo_vein.warning":
    "此处灵气，似有异变；花色未谢，地脉却先冷了一寸。后势将转，留在泉眼旁的人，心口都像被谁记了一笔。",
  "pseudo_vein.dissipate":
    "伪泉眼一息成灰，余灵回灌外圈，另有冷风在百步外结成负压。天道未赐机缘，只把来客照数收走。",
};

export function pseudoVeinNarrationKeyFromSnapshot(
  snapshot: Pick<PseudoVeinSnapshotV1, "spirit_qi_current">,
): PseudoVeinNarrationKey {
  if (snapshot.spirit_qi_current <= 0) {
    return "pseudo_vein.dissipate";
  }
  if (snapshot.spirit_qi_current <= 0.3) {
    return "pseudo_vein.warning";
  }
  return "pseudo_vein.lure";
}

export function renderPseudoVeinSnapshotNarration(snapshot: PseudoVeinSnapshotV1): Narration {
  const key = pseudoVeinNarrationKeyFromSnapshot(snapshot);
  return {
    scope: "zone",
    target: snapshot.id,
    text: PSEUDO_VEIN_NARRATION_TEMPLATES[key],
    style: key === "pseudo_vein.warning" ? "system_warning" : "perception",
  };
}

export function renderPseudoVeinDissipateNarration(event: PseudoVeinDissipateEventV1): Narration {
  return {
    scope: "zone",
    target: event.id,
    text: PSEUDO_VEIN_NARRATION_TEMPLATES["pseudo_vein.dissipate"],
    style: "narration",
  };
}
