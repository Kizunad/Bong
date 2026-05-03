import type { Narration, PlayerProfile, WorldStateV1 } from "@bong/schema";
import { validateNarrationV1Contract } from "@bong/schema";

type QiColorSnapshot = {
  main: string;
  secondary?: string;
  chaotic: boolean;
  hunyuan: boolean;
};

export class QiColorNarrationTracker {
  private readonly seen = new Map<string, QiColorSnapshot>();

  ingest(state: WorldStateV1): Narration[] {
    const narrations: Narration[] = [];

    for (const player of state.players) {
      const current = qiColorSnapshot(player);
      if (!current) continue;

      const previous = this.seen.get(player.uuid);
      this.seen.set(player.uuid, current);
      if (!previous || sameQiColor(previous, current)) continue;

      const narration = renderQiColorNarration(player, current, previous, state.tick);
      if (narration) {
        narrations.push(narration);
      }
    }

    return narrations;
  }
}

export function renderQiColorNarration(
  player: Pick<PlayerProfile, "uuid" | "name">,
  current: QiColorSnapshot,
  previous: QiColorSnapshot,
  tick: number,
): Narration | null {
  const name = shortName(player.name || player.uuid);
  const text = qiColorText(name, current, previous);
  const narration: Narration = {
    scope: "player",
    target: `qi_color:${player.uuid}|tick:${tick}`,
    text,
    style: "narration",
  };

  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

function qiColorSnapshot(player: PlayerProfile): QiColorSnapshot | null {
  const cultivation = player.cultivation;
  if (!cultivation) return null;

  return {
    main: cultivation.qi_color_main,
    secondary: cultivation.qi_color_secondary,
    chaotic: cultivation.qi_color_chaotic,
    hunyuan: cultivation.qi_color_hunyuan,
  };
}

function sameQiColor(left: QiColorSnapshot, right: QiColorSnapshot): boolean {
  return (
    left.main === right.main &&
    left.secondary === right.secondary &&
    left.chaotic === right.chaotic &&
    left.hunyuan === right.hunyuan
  );
}

function qiColorText(name: string, current: QiColorSnapshot, previous: QiColorSnapshot): string {
  if (!previous.hunyuan && current.hunyuan) {
    return `${name} 真元诸色渐归一处，气机转入混元。`;
  }
  if (!previous.chaotic && current.chaotic) {
    return `${name} 真元诸色相争，气机已显杂乱。`;
  }
  if (previous.main !== current.main) {
    return `${name} 真元主色由${colorLabel(previous.main)}转为${colorLabel(current.main)}。`;
  }
  if (previous.secondary !== current.secondary && current.secondary) {
    return `${name} 真元旁色添入${colorLabel(current.secondary)}。`;
  }
  if (previous.chaotic && !current.chaotic) {
    return `${name} 真元杂色稍定，主色仍为${colorLabel(current.main)}。`;
  }
  if (previous.hunyuan && !current.hunyuan) {
    return `${name} 混元气机散开，真元复显${colorLabel(current.main)}。`;
  }
  return `${name} 真元色相微变，主色仍为${colorLabel(current.main)}。`;
}

function colorLabel(color: string): string {
  switch (color) {
    case "Sharp":
      return "锋锐色";
    case "Heavy":
      return "沉重色";
    case "Mellow":
      return "温润色";
    case "Solid":
      return "凝实色";
    case "Light":
      return "飘逸色";
    case "Intricate":
      return "缜密色";
    case "Gentle":
      return "平和色";
    case "Insidious":
      return "阴诡色";
    case "Violent":
      return "暴烈色";
    case "Turbid":
      return "浊乱色";
    default:
      return color;
  }
}

function shortName(id: string): string {
  return id.replace(/^offline:/u, "") || "某修士";
}
