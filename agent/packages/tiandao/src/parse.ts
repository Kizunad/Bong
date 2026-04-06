/**
 * 解析 LLM 输出为结构化指令
 * 容错设计：LLM 输出不稳定时 graceful fallback
 */

import type { Command, Narration } from "@bong/schema";
import { MAX_COMMANDS_PER_TICK, MAX_NARRATION_LENGTH } from "@bong/schema";

export interface AgentDecision {
  commands: Command[];
  narrations: Narration[];
  reasoning: string;
}

const EMPTY_DECISION: AgentDecision = {
  commands: [],
  narrations: [],
  reasoning: "no action",
};

export function parseDecision(raw: string): AgentDecision {
  // 尝试从 markdown code block 中提取 JSON
  const jsonMatch = raw.match(/```(?:json)?\s*\n?([\s\S]*?)\n?```/);
  const jsonStr = jsonMatch ? jsonMatch[1] : raw;

  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(jsonStr.trim());
  } catch {
    console.warn("[tiandao] failed to parse LLM output as JSON, returning empty decision");
    return EMPTY_DECISION;
  }

  const commands = Array.isArray(parsed.commands)
    ? (parsed.commands as Command[]).slice(0, MAX_COMMANDS_PER_TICK)
    : [];

  const narrations = Array.isArray(parsed.narrations)
    ? (parsed.narrations as Narration[]).map((n) => ({
        ...n,
        text: typeof n.text === "string" ? n.text.slice(0, MAX_NARRATION_LENGTH) : "",
      }))
    : [];

  const reasoning =
    typeof parsed.reasoning === "string" ? parsed.reasoning : "no reasoning provided";

  return { commands, narrations, reasoning };
}
