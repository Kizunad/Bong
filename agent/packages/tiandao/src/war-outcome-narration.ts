/**
 * plan-offscreen-war-v1 P9：战事结算匿名叙事 Runtime。
 *
 * 订阅 `bong:faction/war`（`CHANNELS.FACTION_WAR`），仅在 `phase === "settling"` 时
 * emit 一条 broadcast-scope narration（kind: "war_outcome"）。
 *
 * 叙事规则（reframe b）：
 * - 用 `region_descriptor`（匿名「{zone}一带散修」）描述战场，**绝不出现具名宗门**。
 * - 两条模板按 `total_casualties`（高 vs 低）或 outcome 存在性二选一（确定性，测试锁）：
 *   - 模板 A（有 outcome / casualties < 高阈）：
 *     「{region_descriptor}的争斗暂歇，一方势力压过了另一方，谷中灵机似有微动。」
 *   - 模板 B（casualties 高 / outcome 勉强分出）：
 *     「{region_descriptor}厮杀经久方休，胜负之间皆是散修陨落，灵气在余烬里缓缓流转。」
 * - 零真元：narration 纯文本，不触任何 qi 字段。
 */

import {
  CHANNELS,
  type FactionWarEventV1,
  type Narration,
  validateFactionWarEventV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

const { FACTION_WAR, AGENT_NARRATE } = CHANNELS;

/** 高伤亡阈值（total_casualties ≥ 此数选模板 B）。 */
export const WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD = 10;

/**
 * 按 `total_casualties` 和 `winner_group` 确定性选择文本模板。
 * - 有 winner 且伤亡 < 高阈 → 模板 A（速战速决型，一方压倒）。
 * - 无 winner 或伤亡 ≥ 高阈 → 模板 B（久战不决型，惨胜）。
 */
export function selectWarOutcomeTemplate(event: FactionWarEventV1): "A" | "B" {
  const casualties = event.total_casualties ?? 0;
  if (event.winner_group !== undefined && casualties < WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD) {
    return "A";
  }
  return "B";
}

/**
 * 把 FactionWarEventV1（settling 阶段）渲染成战事结算叙事文本。
 * 纯函数，不触任何 qi 字段（守恒：叙事纯文本，零真元）。
 */
export function renderWarOutcomeText(event: FactionWarEventV1): string {
  const desc = event.region_descriptor;
  const template = selectWarOutcomeTemplate(event);
  if (template === "A") {
    return `${desc}的争斗暂歇，一方势力压过了另一方，谷中灵机似有微动。`;
  }
  return `${desc}厮杀经久方休，胜负之间皆是散修陨落，灵气在余烬里缓缓流转。`;
}

/** 把 FactionWarEventV1 转换成 narration 对象（kind: "war_outcome"）。 */
export function renderWarOutcomeNarration(event: FactionWarEventV1): Narration {
  return {
    scope: "broadcast",
    text: renderWarOutcomeText(event),
    style: "perception",
    kind: "war_outcome",
  };
}

export interface WarOutcomeNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface WarOutcomeNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface WarOutcomeNarrationRuntimeConfig {
  sub: WarOutcomeNarrationRuntimeClient;
  pub: WarOutcomeNarrationRuntimeClient;
  logger?: WarOutcomeNarrationRuntimeLogger;
}

export interface WarOutcomeNarrationRuntimeStats {
  /** 收到的 bong:faction/war 事件总数。 */
  received: number;
  /** 成功 emit 的 war_outcome narration 条数。 */
  published: number;
  /** 因 phase != settling 被忽略的事件数。 */
  ignoredPhase: number;
  /** 校验失败（非 JSON / schema 不过）的次数。 */
  rejectedContract: number;
}

export class WarOutcomeNarrationRuntime {
  private readonly sub: WarOutcomeNarrationRuntimeClient;
  private readonly pub: WarOutcomeNarrationRuntimeClient;
  private readonly logger: WarOutcomeNarrationRuntimeLogger;
  private connected = false;

  readonly stats: WarOutcomeNarrationRuntimeStats = {
    received: 0,
    published: 0,
    ignoredPhase: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== FACTION_WAR) return;
    this.handleWarPayload(message);
  };

  constructor(config: WarOutcomeNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(FACTION_WAR);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info("[bong][war-outcome] connected, subscribed to", FACTION_WAR);
  }

  async disconnect(): Promise<void> {
    if (!this.connected) return;
    this.sub.off?.("message", this.onMessage);
    try {
      await this.sub.unsubscribe();
    } catch (_e) {
      // ignore
    }
    // 关闭 Redis 连接（Pi review 2026-06-01 bug fix：防止热重载泄漏 socket）
    try {
      this.sub.disconnect();
    } catch (_e) {
      // ignore
    }
    try {
      this.pub.disconnect();
    } catch (_e) {
      // ignore
    }
    this.connected = false;
  }

  private handleWarPayload(raw: string): void {
    this.stats.received++;

    // 1. JSON parse + schema validate
    let event: FactionWarEventV1;
    try {
      const parsed: unknown = JSON.parse(raw);
      const result = validateFactionWarEventV1Contract(parsed);
      if (!result.ok) {
        this.stats.rejectedContract++;
        this.logger.warn(
          "[bong][war-outcome] rejected invalid payload:",
          result.errors.join("; "),
        );
        return;
      }
      event = parsed as FactionWarEventV1;
    } catch (_e) {
      this.stats.rejectedContract++;
      this.logger.warn("[bong][war-outcome] failed to parse JSON:", _e);
      return;
    }

    // 2. 仅 settling 阶段播报（skirmish/emerging/aftermath 不播，防刷屏）
    if (event.phase !== "settling") {
      this.stats.ignoredPhase++;
      return;
    }

    // 3. 渲染 narration 并 emit（NarrationV1 envelope：{ v: 1, narrations: [...] }）
    const narration = renderWarOutcomeNarration(event);
    const envelope = { v: 1 as const, narrations: [narration] };
    const narrationResult = validateNarrationV1Contract(envelope);
    if (!narrationResult.ok) {
      this.stats.rejectedContract++;
      this.logger.warn(
        "[bong][war-outcome] generated narration failed contract:",
        narrationResult.errors.join("; "),
      );
      return;
    }

    this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope)).then(
      () => {
        this.stats.published++;
        this.logger.info(
          "[bong][war-outcome] published war_outcome narration for zone:",
          event.zone,
        );
      },
      (error: unknown) => {
        this.logger.warn("[bong][war-outcome] publish failed:", error);
      },
    );
  }
}
