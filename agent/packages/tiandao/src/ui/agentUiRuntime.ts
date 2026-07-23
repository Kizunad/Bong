/**
 * plan-agent-ui-data-v1 P2 — AgentUiRuntime
 *
 * 生产路径的 UiRenderer + UiResponseConsumer 接线层。
 *
 * 职责：
 *   1. 内部持有 UiRenderer（发 AgentUiRequestCommandV1）和 UiResponseConsumer（消费响应）
 *   2. UiResponseConsumer.onButtonClick 回调把 button_click 追加到 pendingButtonClicks 队列，
 *      供下一轮推演（Arbiter tick）drain 消费
 *   3. triggerUi() 是生产触发入口，调用 UiRenderer.renderUi() 并返回 requestId
 *
 * 生命周期：connect() → triggerUi*()/drainPendingClicks*() → disconnect()
 */

import type { AgentUiResponsePayloadV1 } from "@bong/schema";
import { UiRenderer } from "./uiRenderer.js";
import type { RenderUiOptions, RenderUiResult } from "./uiRenderer.js";
import { UiResponseConsumer } from "./uiResponseConsumer.js";

// ─── 接口 ────────────────────────────────────────────────────────────────────

export interface AgentUiRuntimePubClient {
  publish(channel: string, message: string): Promise<number>;
}

export interface AgentUiRuntimeSubClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
}

export interface AgentUiRuntimeConfig {
  /** Redis pub（发布 bong:agent_ui_cmd，也发布 bong:agent_narrate） */
  pub: AgentUiRuntimePubClient;
  /** Redis sub（仅订阅 bong:agent_ui_response） */
  sub: AgentUiRuntimeSubClient;
  logger?: { info: (...args: unknown[]) => void; warn: (...args: unknown[]) => void };
}

// ─── AgentUiRuntime 类 ────────────────────────────────────────────────────────

export class AgentUiRuntime {
  private readonly renderer: UiRenderer;
  private readonly consumer: UiResponseConsumer;
  /** button_click 事件队列，供 Arbiter 下一轮推演 drain */
  private readonly pendingButtonClicks: AgentUiResponsePayloadV1[] = [];
  /** session 结束事件队列（dismissed / timeout / completed） */
  private readonly pendingSessionEnds: AgentUiResponsePayloadV1[] = [];

  constructor(config: AgentUiRuntimeConfig) {
    if (Object.is(config.pub, config.sub)) {
      throw new Error("AgentUiRuntime publisher must be distinct from subscriber");
    }

    const logger = config.logger ?? {
      info: (...args: unknown[]) => console.log("[agent-ui-runtime]", ...args),
      warn: (...args: unknown[]) => console.warn("[agent-ui-runtime]", ...args),
    };

    this.renderer = new UiRenderer({
      pub: config.pub,
      logger,
    });

    // 命令与 narration 都是普通 Redis publish，可安全复用同一发布连接；
    // 订阅连接进入 subscriber mode 后绝不参与 publish。
    this.consumer = new UiResponseConsumer({
      sub: config.sub,
      pub: config.pub,
      logger,
      onButtonClick: (response) => {
        this.pendingButtonClicks.push(response);
        logger.info(
          `[agent-ui-runtime] queued button_click request_id=${response.request_id} ` +
          `button_id=${response.params["button_id"] ?? "(none)"} ` +
          `queue_depth=${this.pendingButtonClicks.length}`,
        );
      },
      onSessionEnd: (response) => {
        this.pendingSessionEnds.push(response);
        logger.info(
          `[agent-ui-runtime] queued session_end action=${response.action} ` +
          `request_id=${response.request_id}`,
        );
      },
    });
  }

  /** 订阅 bong:agent_ui_response，开始消费响应。 */
  async connect(): Promise<void> {
    await this.consumer.connect();
  }

  /** 断开逻辑订阅；physical Redis client 由创建方关闭。 */
  async disconnect(): Promise<void> {
    await this.consumer.disconnect();
  }

  /**
   * 触发 UI 面板生成并发布到 bong:agent_ui_cmd。
   *
   * 生产触发时机（由外部 hook 决定，如：
   *   - tick 后 Arbiter 输出含 tsy_discovery spawn_event 时
   *   - elder_encounter 事件到达时
   *   - 天道启示决策时
   * ）
   */
  async triggerUi(options: RenderUiOptions): Promise<RenderUiResult> {
    return this.renderer.renderUi(options);
  }

  /**
   * drain 所有待处理的 button_click 事件（供 Arbiter 下一轮推演注入）。
   * 调用后队列清空。
   */
  drainPendingButtonClicks(): AgentUiResponsePayloadV1[] {
    return this.pendingButtonClicks.splice(0);
  }

  /**
   * drain 所有待处理的 session 结束事件（dismissed / timeout / completed）。
   * 调用后队列清空。
   */
  drainPendingSessionEnds(): AgentUiResponsePayloadV1[] {
    return this.pendingSessionEnds.splice(0);
  }

  /** UiResponseConsumer 的统计信息（供监控 / 测试断言） */
  get stats() {
    return this.consumer.stats;
  }
}
