import {
  CHANNELS,
  type FactionStateV1,
  validateFactionStateV1Contract,
} from "@bong/schema";

const { FACTION_STATE } = CHANNELS;

/**
 * plan-offscreen-war-v1 P5：散修群体消长盘面 census 薄消费层。
 *
 * 订阅 `bong:faction_state`，校验后按 group_id 存「最新每组 census」。
 * **纯只读 census**——不触碰 WorldQiAccount / ledger / 战斗路径，守恒红线（§10.1 #5）。
 * **匿名涌现框架**：不引入具名宗门，group_id 是裸数字，region_descriptor 是
 * `"{zone}一带散修"` 式区域描述符（hardcoded 于 server 侧）。
 */

export interface FactionCensusStoreClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
}

export interface FactionCensusStoreLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface FactionCensusStoreConfig {
  sub: FactionCensusStoreClient;
  logger?: FactionCensusStoreLogger;
}

export interface FactionCensusStoreStats {
  /** 收到的 bong:faction_state 消息总数（含被拒绝的）。 */
  received: number;
  /** 校验通过并存入 census 的条数。 */
  accepted: number;
  /** 校验失败（非 JSON / 契约不过）的次数。 */
  rejected: number;
}

export class FactionCensusStore {
  private readonly sub: FactionCensusStoreClient;
  private readonly logger: FactionCensusStoreLogger;
  private connected = false;

  /**
   * 最新每组 census：key = group_id（number），value = 最新 FactionStateV1 快照。
   * 每次收到同 group_id 的更新直接覆盖，保证每 group 只保留最新状态。
   */
  private readonly census = new Map<number, FactionStateV1>();

  readonly stats: FactionCensusStoreStats = {
    received: 0,
    accepted: 0,
    rejected: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== FACTION_STATE) return;
    this.handleFactionStatePayload(message);
  };

  constructor(config: FactionCensusStoreConfig) {
    this.sub = config.sub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(FACTION_STATE);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[faction-census-store] subscribed to ${FACTION_STATE}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
  }

  /**
   * 处理单条 `bong:faction_state` wire payload：校验契约 → 存入 census。
   * 解析或校验失败时记 stats.rejected + warn，不抛异常。
   */
  handleFactionStatePayload(message: string): void {
    this.stats.received += 1;
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejected += 1;
      this.logger.warn("[faction-census-store] non-JSON payload:", error);
      return;
    }

    const validation = validateFactionStateV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejected += 1;
      this.logger.warn(
        "[faction-census-store] FactionStateV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    const state = parsed as FactionStateV1;
    this.census.set(state.group_id, state);
    this.stats.accepted += 1;
  }

  /**
   * 获取指定 group_id 的最新 census 快照。无记录时返回 undefined。
   */
  getGroupCensus(groupId: number): FactionStateV1 | undefined {
    return this.census.get(groupId);
  }

  /**
   * 获取所有已知群体的最新 census 快照（副本，防止外部修改内部 map）。
   */
  getAllCensus(): Map<number, FactionStateV1> {
    return new Map(this.census);
  }

  /**
   * 返回当前 census 中存活群体数（== group 数，非 population 总和）。
   */
  get groupCount(): number {
    return this.census.size;
  }
}
