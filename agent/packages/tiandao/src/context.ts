/**
 * Context Assembler — 模块化上下文拼装引擎
 * 按 Agent 角色裁剪 world state 为结构化 prompt
 */

import type { WorldStateV1, PlayerProfile, ZoneSnapshot, GameEvent } from "@bong/schema";

export interface ContextBlock {
  name: string;
  priority: number; // 0=最高，越大越容易被裁剪
  required: boolean;
  render: (state: WorldStateV1) => string;
}

export interface ContextRecipe {
  agentName: string;
  blocks: ContextBlock[];
  maxTokenEstimate: number; // 粗估 token 上限 (1 token ≈ 4 chars 中英混合按 2 chars)
}

function estimateTokens(text: string): number {
  // 粗估：中英文混合平均 2 chars/token
  return Math.ceil(text.length / 2);
}

export function assembleContext(
  recipe: ContextRecipe,
  state: WorldStateV1,
): string {
  const rendered: { priority: number; required: boolean; text: string }[] = [];

  for (const block of recipe.blocks) {
    const text = block.render(state);
    if (text) {
      rendered.push({ priority: block.priority, required: block.required, text });
    }
  }

  // 按 priority 排序（低数字 = 高优先）
  rendered.sort((a, b) => a.priority - b.priority);

  // 逐步拼装，超预算时裁剪非必需
  let total = 0;
  const included: string[] = [];

  for (const item of rendered) {
    const tokens = estimateTokens(item.text);
    if (total + tokens > recipe.maxTokenEstimate && !item.required) {
      continue;
    }
    included.push(item.text);
    total += tokens;
  }

  return included.join("\n\n---\n\n");
}

// ─── 预置 Context Blocks ─────────────────────────────────

export const worldSnapshotBlock: ContextBlock = {
  name: "world_snapshot",
  priority: 1,
  required: true,
  render(state) {
    const zones = state.zones
      .map((z: ZoneSnapshot) => `- ${z.name}: 灵气 ${z.spirit_qi.toFixed(2)}, 危险 ${z.danger_level}/5, 玩家 ${z.player_count}人`)
      .join("\n");
    return `## 世界快照\nTick: ${state.tick}, 在线: ${state.players.length}人\n\n${zones}`;
  },
};

export const playerProfilesBlock: ContextBlock = {
  name: "player_profiles",
  priority: 1,
  required: true,
  render(state) {
    if (state.players.length === 0) return "";
    const header = "| 玩家 | 综合实力 | 战斗 | karma | 趋势 | 位置 |";
    const sep = "|------|---------|------|-------|------|------|";
    const rows = state.players.map((p: PlayerProfile) => {
      const trend = p.trend === "rising" ? "↑" : p.trend === "falling" ? "↓" : "→";
      return `| ${p.name} | ${p.composite_power.toFixed(2)} | ${p.breakdown.combat.toFixed(2)} | ${p.breakdown.karma.toFixed(2)} | ${trend} | ${p.zone} |`;
    });
    return `## 玩家画像\n${header}\n${sep}\n${rows.join("\n")}`;
  },
};

export const recentEventsBlock: ContextBlock = {
  name: "recent_events",
  priority: 2,
  required: false,
  render(state) {
    if (state.recent_events.length === 0) return "";
    const lines = state.recent_events
      .slice(-10)
      .map((e: GameEvent) => {
        const parts = [`[tick ${e.tick}] ${e.type}`];
        if (e.player) parts.push(e.player);
        if (e.zone) parts.push(`@ ${e.zone}`);
        return parts.join(" ");
      });
    return `## 近期事件\n${lines.join("\n")}`;
  },
};

// ─── 预置 Recipes ─────────────────────────────────────────

export const CALAMITY_RECIPE: ContextRecipe = {
  agentName: "calamity",
  maxTokenEstimate: 3000,
  blocks: [
    { ...playerProfilesBlock, priority: 0, required: true },
    { ...recentEventsBlock, priority: 1, required: true },
    { ...worldSnapshotBlock, priority: 2, required: false },
  ],
};

export const MUTATION_RECIPE: ContextRecipe = {
  agentName: "mutation",
  maxTokenEstimate: 3000,
  blocks: [
    { ...worldSnapshotBlock, priority: 0, required: true },
    { ...playerProfilesBlock, priority: 1, required: true },
    { ...recentEventsBlock, priority: 2, required: false },
  ],
};

export const ERA_RECIPE: ContextRecipe = {
  agentName: "era",
  maxTokenEstimate: 4000,
  blocks: [
    { ...worldSnapshotBlock, priority: 0, required: true },
    { ...playerProfilesBlock, priority: 1, required: true },
    { ...recentEventsBlock, priority: 1, required: true },
  ],
};
