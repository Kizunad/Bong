import type {
  Narration,
  PoisonDoseEventV1,
  PoisonOverdoseEventV1,
  PoisonSideEffectTagV1,
} from "@bong/schema";

const SIDE_EFFECT_TEXT: Record<PoisonSideEffectTagV1, string> = {
  qi_focus_drift_2h: "眼前准星微偏，真元像被一层灰雾拖住。",
  rage_burst_30min: "血气忽然上冲，力道涨了半分，步子却沉了一截。",
  hallucin_tint_6h: "视野边角泛起青鳞般的影，远近一时难辨。",
  digest_lock_6h: "丹毒锁在腹中不散，像一枚冷钉压着胃火。",
  toxicity_tier_unlock: "经络里有一线暗绿沉下去，附毒的门槛被硬生生推开。",
};

export function poisonSideEffectText(tag: PoisonSideEffectTagV1): string {
  return SIDE_EFFECT_TEXT[tag];
}

export function renderPoisonDoseNarration(event: PoisonDoseEventV1): Narration {
  const text = `毒丹入腹，毒性真元升至 ${event.poison_level_after.toFixed(0)}，消化负荷压到 ${event.digestion_after.toFixed(0)}。${poisonSideEffectText(event.side_effect_tag)}`;
  return {
    scope: "player",
    target: `poison_dose:${event.player_entity_id}|tick:${event.at_tick}`,
    text,
    style: "narration",
  };
}

export function renderPoisonOverdoseNarration(event: PoisonOverdoseEventV1): Narration {
  const severityText = {
    mild: "轻微反噬",
    moderate: "中度反噬",
    severe: "重度反噬",
  }[event.severity];
  const tearText =
    event.micro_tear_probability > 0
      ? "经脉边缘传来细响，像旧瓷又添了一道微裂。"
      : "经脉暂未裂开，只是余毒还在腹中打转。";
  return {
    scope: "player",
    target: `poison_overdose:${event.player_entity_id}|tick:${event.at_tick}`,
    text: `${severityText}压住气息，寿元折去 ${event.lifespan_penalty_years.toFixed(1)} 年。${tearText}`,
    style: "narration",
  };
}
