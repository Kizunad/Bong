/**
 * plan-agent-ui-data-v1 P2 — UiRenderer
 *
 * 根据当前 WorldStateV1 + 指定玩家的 realm，选择适当面板模板、设置 realm_gate，
 * 生成 AgentUiRequestCommandV1 并发布到 Redis `bong:agent_ui_cmd`。
 *
 * realm_gate 规则（设计决议 §0.1）：
 *   - 天道启示   realm_gate=5（通灵境+）
 *   - 活坍缩渊   realm_gate=3（凝脉境+）
 *   - 一般面板   realm_gate=0（不门控）
 *
 * 当 player.realm < realm_gate 时，server 会拒绝并向 bong:agent_ui_response 发布
 * { action: "error", params: { reason: "realm_gate_rejected", ... } }；
 * uiResponseConsumer 收到后 emit narration 替代面板（见 §8 Q3）。
 *
 * 境界不足模糊版（UX 降级）：
 *   当 agent 侧 player.realm rank < realm_gate 时，agent 主动生成模糊版 XML
 *   （realm_gate=0，内容已模糊），server 不会拒绝，client 只看到模糊感应文字。
 */

import type { AgentUiRequestCommandV1 } from "@bong/schema";
import { CHANNELS } from "@bong/schema";
import type { PlayerProfile, WorldStateV1 } from "@bong/schema";
import { randomUUID } from "node:crypto";
import {
  BLUR_INSUFFICIENT_TEMPLATE,
  TEMPLATE_REALM_GATE,
  extractButtonIds,
  realmStringToRank,
  renderTemplate,
  xmlEscape,
} from "./xmlTemplates.js";

const { AGENT_UI_CMD } = CHANNELS;

// ─── 接口 ────────────────────────────────────────────────────────────────────

export interface UiRendererClient {
  publish(channel: string, message: string): Promise<number>;
}

export interface UiRendererLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface UiRendererConfig {
  pub: UiRendererClient;
  logger?: UiRendererLogger;
  /** 用于生成 request_id 的函数（默认 uuidv4） */
  generateRequestId?: () => string;
  /** 用于获取当前时间戳（ms）的函数（默认 Date.now） */
  now?: () => number;
}

/** 面板触发场景 */
export type UiScenario =
  | "tsy_discovery"      // 活坍缩渊秘境发现
  | "elder_legacy"       // 垂死大能传承
  | "tiandao_revelation" // 天道启示

export interface RenderUiOptions {
  scenario: UiScenario;
  targetPlayer: PlayerProfile;
  /** 额外模板参数（具体 scenario 所需字段） */
  params: Record<string, unknown>;
  /** override timeout_ticks（默认 600） */
  timeoutTicks?: number;
}

export interface RenderUiResult {
  requestId: string;
  command: AgentUiRequestCommandV1;
  /** true = 发布了清晰版（realm 足够）；false = 发布了模糊版（realm 不足） */
  sentBlurVersion: boolean;
}

// ─── UiRenderer 类 ───────────────────────────────────────────────────────────

export class UiRenderer {
  private readonly pub: UiRendererClient;
  private readonly logger: UiRendererLogger;
  private readonly generateRequestId: () => string;
  private readonly now: () => number;

  constructor(config: UiRendererConfig) {
    this.pub = config.pub;
    this.logger = config.logger ?? {
      info: (...args) => console.log("[ui-renderer]", ...args),
      warn: (...args) => console.warn("[ui-renderer]", ...args),
    };
    this.generateRequestId = config.generateRequestId ?? (() => randomUUID());
    this.now = config.now ?? (() => Date.now());
  }

  /**
   * 根据 scenario + player.realm 选模板、设 realm_gate，发布 AgentUiRequestCommandV1。
   *
   * 当 player.realm rank < scenario 的 realm_gate 时，发布模糊版（UX 降级）：
   *   - 模糊版 realm_gate=0（不门控），内容已模糊
   *   - 不发完整面板（避免 server realm_gate 拒绝后 agent 只收到 error event）
   *
   * @throws {Error} 如果 params 缺少必填参数
   */
  async renderUi(options: RenderUiOptions): Promise<RenderUiResult> {
    const { scenario, targetPlayer, params, timeoutTicks = 600 } = options;

    const playerRealmRank = realmStringToRank(targetPlayer.realm);
    const scenarioRealmGate = TEMPLATE_REALM_GATE[scenario];
    const isRealmInsufficient = playerRealmRank < scenarioRealmGate;

    let xml: string;
    let realmGate: number;
    let allowedButtonIds: string[];
    let sentBlurVersion: boolean;

    if (isRealmInsufficient) {
      // 境界不足 → 模糊版
      xml = renderBlurVersion(scenario, targetPlayer.realm ?? "");
      realmGate = 0;
      allowedButtonIds = ["dismiss"];
      sentBlurVersion = true;
    } else {
      // 境界足够 → 清晰版
      xml = renderTemplate({ kind: scenario, params });
      realmGate = scenarioRealmGate;
      allowedButtonIds = extractButtonIds(xml);
      sentBlurVersion = false;
    }

    const requestId = this.generateRequestId();
    const command: AgentUiRequestCommandV1 = {
      request_id: requestId,
      target_player: targetPlayer.uuid,
      xml,
      timeout_ticks: timeoutTicks,
      realm_gate: realmGate,
      allowed_button_ids: allowedButtonIds,
    };

    try {
      const subscribers = await this.pub.publish(AGENT_UI_CMD, JSON.stringify(command));
      this.logger.info(
        `[ui-renderer] published AgentUiRequestCommandV1 request_id=${requestId} ` +
        `scenario=${scenario} target=${targetPlayer.name} ` +
        `realm_gate=${realmGate} blur=${sentBlurVersion} subscribers=${subscribers}`,
      );
    } catch (error) {
      this.logger.warn("[ui-renderer] failed to publish AgentUiRequestCommandV1:", error);
      throw error;
    }

    return { requestId, command, sentBlurVersion };
  }

  /**
   * 根据世界状态 + 场景自动选择目标玩家（选第一个在线玩家，仅供 demo/测试用）。
   * 生产场景由调用方显式传入 targetPlayer。
   */
  selectTargetFromState(state: WorldStateV1, scenario: UiScenario): PlayerProfile | null {
    if (state.players.length === 0) {
      return null;
    }
    // 天道启示优先选通灵+境玩家（如有）
    if (scenario === "tiandao_revelation") {
      const spiritPlus = state.players.find((p) => {
        const rank = realmStringToRank(p.realm);
        return rank >= 5;
      });
      if (spiritPlus) return spiritPlus;
    }
    return state.players[0];
  }
}

// ─── 内部辅助 ────────────────────────────────────────────────────────────────

/** 生成境界不足模糊版 XML */
function renderBlurVersion(scenario: UiScenario, realm: string): string {
  const blurHints: Record<UiScenario, string> = {
    tsy_discovery:      "感知到一丝异常涌动，境界不足以辨明其真实面貌",
    elder_legacy:       "有什么低语试图传入，却被境界隔绝在外",
    tiandao_revelation: "天道注意力一闪而过，缘浅，无法承受",
  };
  const hint = blurHints[scenario] ?? "有什么正在发生，你感知到了一丝余韵";
  // blur_insufficient 模板不通过 renderTemplate（它用的是 BLUR_INSUFFICIENT_TEMPLATE）
  return BLUR_INSUFFICIENT_TEMPLATE
    .replace(/\{\{blur_hint\}\}/g, xmlEscape(hint));
}
