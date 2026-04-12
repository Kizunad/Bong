/**
 * 解析 LLM 输出为结构化指令
 * 容错设计：LLM 输出不稳定时 graceful fallback
 */

import type { Command, Narration } from "@bong/schema";
import {
  MAX_COMMANDS_PER_TICK,
  MAX_NARRATION_LENGTH,
  validateAgentCommandV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

interface ParseFailureSummary {
  commands: number;
  narrations: number;
  total: number;
}

export interface AgentDecision {
  commands: Command[];
  narrations: Narration[];
  reasoning: string;
  parseFailures?: ParseFailureSummary;
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

  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonStr.trim());
  } catch {
    console.warn("[tiandao] failed to parse LLM output as JSON, returning empty decision");
    return EMPTY_DECISION;
  }

  const decision = isRecord(parsed) ? parsed : {};

  const commands: Command[] = [];
  let commandFailures = 0;
  if (Array.isArray(decision.commands)) {
    for (const candidate of decision.commands) {
      const validation = validateAgentCommandV1Contract({
        v: 1,
        id: "cmd_parse_candidate",
        source: "arbiter",
        commands: [candidate],
      });
      if (!validation.ok) {
        commandFailures += 1;
        console.warn(`[tiandao] dropped invalid command row: ${validation.errors.join("; ")}`);
        continue;
      }

      if (commands.length < MAX_COMMANDS_PER_TICK) {
        commands.push(candidate as Command);
      }
    }
  }

  const narrations: Narration[] = [];
  let narrationFailures = 0;
  if (Array.isArray(decision.narrations)) {
    for (const candidate of decision.narrations) {
      const normalizedNarration = normalizeNarrationCandidate(candidate);
      const validation = validateNarrationV1Contract({
        v: 1,
        narrations: [normalizedNarration],
      });
      if (!validation.ok) {
        narrationFailures += 1;
        console.warn(`[tiandao] dropped invalid narration row: ${validation.errors.join("; ")}`);
        continue;
      }

      narrations.push(normalizedNarration as Narration);
    }
  }

  const reasoning = typeof decision.reasoning === "string" ? decision.reasoning : "no reasoning provided";
  const totalFailures = commandFailures + narrationFailures;

  if (totalFailures === 0) {
    return { commands, narrations, reasoning };
  }

  return {
    commands,
    narrations,
    reasoning,
    parseFailures: {
      commands: commandFailures,
      narrations: narrationFailures,
      total: totalFailures,
    },
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeNarrationCandidate(candidate: unknown): unknown {
  if (!isRecord(candidate)) {
    return candidate;
  }

  const normalized: Record<string, unknown> = { ...candidate };
  if (typeof normalized.text === "string") {
    normalized.text = normalized.text.slice(0, MAX_NARRATION_LENGTH);
  }

  return normalized;
}
