/**
 * plan-agent-ui-data-v1 P2 — UiResponseConsumer
 *
 * 订阅 Redis `bong:agent_ui_response`，消费 AgentUiResponsePayloadV1：
 *
 * 处理逻辑（设计决议 §8 Q3）：
 *   - button_click  → 触发注入 Arbiter 的回调（外部传入，供 runtime 在下一轮推演中处理）
 *   - dismissed     → 标记会话结束（外部回调）
 *   - timeout       → 标记会话结束（外部回调）
 *   - replaced      → 静默（server 已发 close，client 已关闭面板）
 *   - parse_error   → 记录警告日志
 *   - error + reason=realm_gate_rejected → emit narration（复用 bong:agent_narrate）
 *   - error + reason=player_offline      → 记录警告日志
 *   - error + reason=invalid_button_id   → 记录警告日志
 *   - error (其余 reason)               → 记录警告日志
 *
 * narration 格式（realm_gate_rejected）：
 *   scope="player"，target=player_uuid（若无则 scope="broadcast"，target="world"）
 *   style="system_warning"
 *   text 文本来自 REALM_GATE_NARRATION_TEXT
 */

import {
  CHANNELS,
  type AgentUiResponsePayloadV1,
  type Narration,
  validateAgentUiResponsePayloadV1Contract,
} from "@bong/schema";

const { AGENT_UI_RESPONSE, AGENT_NARRATE } = CHANNELS;

// ─── 接口 ────────────────────────────────────────────────────────────────────

export interface UiResponseConsumerClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface UiResponseConsumerLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

/**
 * button_click 事件的 Arbiter 注入回调。
 * 供 runtime 在下一轮推演时感知玩家选择。
 */
export type ButtonClickCallback = (response: AgentUiResponsePayloadV1) => void;

/**
 * 会话结束（dismissed / timeout / completed）回调。
 */
export type SessionEndCallback = (response: AgentUiResponsePayloadV1) => void;

export interface UiResponseConsumerConfig {
  sub: UiResponseConsumerClient;
  pub: UiResponseConsumerClient;
  logger?: UiResponseConsumerLogger;
  /** button_click 回调（注入 Arbiter 推演） */
  onButtonClick?: ButtonClickCallback;
  /** 会话结束回调（dismissed / timeout） */
  onSessionEnd?: SessionEndCallback;
}

export interface UiResponseConsumerStats {
  received: number;
  buttonClick: number;
  dismissed: number;
  timeout: number;
  replaced: number;
  realmGateRejected: number;
  parseError: number;
  otherError: number;
  rejectedContract: number;
  narrationPublished: number;
}

// ─── realm_gate_rejected narration 文本 ──────────────────────────────────────

/** realm_gate_rejected 时向玩家发布的叙事文本 */
export const REALM_GATE_NARRATION_TEXT =
  "天道的注意力掠过，境界未至，缘分尚浅——此时感知到的，只是一片模糊的余韵。";

// ─── UiResponseConsumer 类 ────────────────────────────────────────────────────

export class UiResponseConsumer {
  private readonly sub: UiResponseConsumerClient;
  private readonly pub: UiResponseConsumerClient;
  private readonly logger: UiResponseConsumerLogger;
  private readonly onButtonClick: ButtonClickCallback | undefined;
  private readonly onSessionEnd: SessionEndCallback | undefined;
  private connected = false;

  readonly stats: UiResponseConsumerStats = {
    received: 0,
    buttonClick: 0,
    dismissed: 0,
    timeout: 0,
    replaced: 0,
    realmGateRejected: 0,
    parseError: 0,
    otherError: 0,
    rejectedContract: 0,
    narrationPublished: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== AGENT_UI_RESPONSE) return;
    void this.handlePayload(message);
  };

  constructor(config: UiResponseConsumerConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? {
      info: (...args) => console.log("[ui-response-consumer]", ...args),
      warn: (...args) => console.warn("[ui-response-consumer]", ...args),
    };
    this.onButtonClick = config.onButtonClick;
    this.onSessionEnd = config.onSessionEnd;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(AGENT_UI_RESPONSE);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[ui-response-consumer] subscribed to ${AGENT_UI_RESPONSE}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[ui-response-consumer] non-JSON payload:", error);
      return;
    }

    const validation = validateAgentUiResponsePayloadV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[ui-response-consumer] invalid AgentUiResponsePayloadV1:",
        validation.errors.join("; "),
      );
      return;
    }

    this.stats.received += 1;
    const response = parsed as AgentUiResponsePayloadV1;
    await this.dispatch(response);
  }

  private async dispatch(response: AgentUiResponsePayloadV1): Promise<void> {
    switch (response.action) {
      case "button_click":
        this.stats.buttonClick += 1;
        this.logger.info(
          `[ui-response-consumer] button_click request_id=${response.request_id} ` +
          `button_id=${response.params["button_id"] ?? "(none)"}`,
        );
        this.onButtonClick?.(response);
        break;

      case "dismissed":
        this.stats.dismissed += 1;
        this.logger.info(
          `[ui-response-consumer] dismissed request_id=${response.request_id}`,
        );
        this.onSessionEnd?.(response);
        break;

      case "timeout":
        this.stats.timeout += 1;
        this.logger.info(
          `[ui-response-consumer] timeout request_id=${response.request_id}`,
        );
        this.onSessionEnd?.(response);
        break;

      case "replaced":
        // 静默：server 已发 close，client 已关闭面板，无需额外处理
        this.stats.replaced += 1;
        break;

      case "parse_error":
        this.stats.parseError += 1;
        this.logger.warn(
          `[ui-response-consumer] client parse_error request_id=${response.request_id}`,
        );
        break;

      case "error":
        await this.handleError(response);
        break;

      default:
        this.logger.warn(
          `[ui-response-consumer] unknown action "${(response as AgentUiResponsePayloadV1).action}" ` +
          `request_id=${response.request_id}`,
        );
    }
  }

  private async handleError(response: AgentUiResponsePayloadV1): Promise<void> {
    const reason = response.params["reason"] ?? "";

    if (reason === "realm_gate_rejected") {
      this.stats.realmGateRejected += 1;
      await this.emitRealmGateRejectedNarration(response);
    } else if (reason === "player_offline") {
      this.logger.warn(
        `[ui-response-consumer] player_offline request_id=${response.request_id}`,
      );
      this.stats.otherError += 1;
    } else if (reason === "invalid_button_id") {
      this.logger.warn(
        `[ui-response-consumer] invalid_button_id request_id=${response.request_id}`,
      );
      this.stats.otherError += 1;
    } else {
      this.logger.warn(
        `[ui-response-consumer] error request_id=${response.request_id} reason=${reason}`,
      );
      this.stats.otherError += 1;
    }
  }

  /**
   * realm_gate_rejected → emit narration（发布到 bong:agent_narrate）。
   *
   * 设计决议 §8 Q3：v1 只走 narration，使用 bong:agent_narrate 复用现有 narration 管道。
   */
  private async emitRealmGateRejectedNarration(
    response: AgentUiResponsePayloadV1,
  ): Promise<void> {
    // 由于 AgentUiResponsePayloadV1 不含 target_player 字段（server→agent Redis 同构），
    // 只能从 params 推断（无标准 target_player 字段），因此使用 broadcast scope。
    const narration: Narration = {
      scope: "broadcast",
      target: "world",
      style: "system_warning",
      text: REALM_GATE_NARRATION_TEXT,
    };

    try {
      const payload = JSON.stringify({ v: 1, narrations: [narration] });
      await this.pub.publish(AGENT_NARRATE, payload);
      this.stats.narrationPublished += 1;
      this.logger.info(
        `[ui-response-consumer] emitted realm_gate_rejected narration request_id=${response.request_id}`,
      );
    } catch (error) {
      this.logger.warn(
        "[ui-response-consumer] failed to publish realm_gate_rejected narration:",
        error,
      );
    }
  }
}
