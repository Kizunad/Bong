import {
  CHANNELS,
  type FactionIdV1,
  type Narration,
  type NpcDeathV1,
  validateNarrationV1Contract,
  validateNpcDeathV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, NPC_DEATH } = CHANNELS;

/**
 * plan-offscreen-war-v1 P4：天道感知远方离屏战事。
 *
 * 复用 `bong:npc/death`（带 `from_dormant_combat` + `faction_id` + `pos`，P0 落地）——
 * **不新建 telemetry channel**（v3 骨架已明确 agent 无需额外通道，新 channel 是孤岛红旗，见
 * docs/CLAUDE.md §四）。本 runtime 在一个推演窗口内把 dormant 派系互殴战死按 (region, faction_id)
 * 聚合，仅当窗口存在 `from_dormant_combat===true` 的 combat 战死时 emit 一条 broadcast-scope
 * perception narration（散修群体涌现消长叙事）。`natural_aging` 等非 combat 死亡不进战报。
 *
 * 注意：`NpcDeathV1` 线上**没有 `zone` 字段**（zone 只在 `NpcSpawnedV1` / `bong:world_state`
 * 上），死亡事件只带 `pos: [x,y,z]`（P0 加，dormant 战死回填 `snapshot.position`）。本 P4 是
 * 纯 agent 侧消费——不改 server / schema——故聚合维度用 **pos 派生的粗粒度 region 网格**，而非
 * 精确 zone 名。天道"感知远方战事"本就是方位/区域级的模糊感知（非测绘级 zone 名），region 网格
 * 与该基调一致；`pos=None` 的死亡落到一个稳定的「unknown」region 桶。
 *
 * WORLDVIEW 锚点（docs/worldview.md §七 散修利己竞争 + §十 灵气零和）：叙事**只用匿名 / 散修群体
 * 涌现框架**（「两方散修」「某派系」「无名之辈」），**绝不出现具名宗门**——具名势力是 P5 的层，
 * 缺 worldview 锚点已 gated（见 plan §10.1 #6，倾向方案 (b) 散修群体涌现 reframe）。
 */

/**
 * region 网格边长（方块数）。死亡坐标按此向下取整量化成 region cell，让"同一方圆几百格内"的
 * 离屏战死聚到一处。默认 256（约一个大地形分区粒度），既能把一场互殴的尸体聚拢，又不至于把
 * 全图战死合并成一坨。导出供测试 pin。
 */
export const OFFSCREEN_WAR_REGION_CELL_SIZE = 256;

/** pos=None 死亡的稳定 region 占位桶。 */
export const UNKNOWN_REGION_KEY = "region:unknown";

/** 进战报的门槛：dormant 派系互殴所致的 combat 战死。 */
export function isOffscreenCombatDeath(event: NpcDeathV1): boolean {
  return event.from_dormant_combat === true && event.cause === "combat";
}

/**
 * 把死亡坐标量化成稳定的 region key（按 X/Z 平面网格，忽略 Y 高度——离屏战事是平面方位感知）。
 * 无坐标（pos=undefined）→ {@link UNKNOWN_REGION_KEY}。
 * 形如 `region:0,-1`（cell 索引），确定性、可排序。
 */
export function offscreenWarRegionKey(
  pos: readonly [number, number, number] | undefined,
  cellSize: number = OFFSCREEN_WAR_REGION_CELL_SIZE,
): string {
  if (!pos) return UNKNOWN_REGION_KEY;
  const cx = Math.floor(pos[0] / cellSize);
  const cz = Math.floor(pos[2] / cellSize);
  return `region:${cx},${cz}`;
}

/** 聚合维度键：region + faction_id（无 faction_id 落到匿名占位）。 */
export function offscreenWarGroupKey(
  regionKey: string,
  factionId: FactionIdV1 | undefined,
): string {
  return `${regionKey}|${factionId ?? "unknown"}`;
}

export interface OffscreenWarFactionTally {
  regionKey: string;
  factionId: FactionIdV1 | undefined;
  deaths: number;
}

export interface OffscreenWarReport {
  /** 进战报的 combat 战死总数（已过滤 natural_aging 等）。 */
  totalCombatDeaths: number;
  /** 涉及的 region 集合（去重，按首见顺序）。 */
  regions: string[];
  /** 按 (region, faction_id) 分组的战损统计，按 group key 排序，结果确定。 */
  factionTallies: OffscreenWarFactionTally[];
}

/**
 * 把一批 death 事件聚合成派系战报：过滤非 combat、按 (region, faction_id) group_by。
 * 纯函数，确定性——Context Assembler（喂三 Agent）与本 runtime（emit narration）共用。
 */
export function aggregateOffscreenWarReport(
  events: readonly NpcDeathV1[],
  cellSize: number = OFFSCREEN_WAR_REGION_CELL_SIZE,
): OffscreenWarReport {
  const combatDeaths = events.filter(isOffscreenCombatDeath);
  const regions: string[] = [];
  const seenRegions = new Set<string>();
  const tallyByKey = new Map<string, OffscreenWarFactionTally>();

  for (const event of combatDeaths) {
    const regionKey = offscreenWarRegionKey(event.pos, cellSize);
    if (!seenRegions.has(regionKey)) {
      seenRegions.add(regionKey);
      regions.push(regionKey);
    }
    const key = offscreenWarGroupKey(regionKey, event.faction_id);
    const existing = tallyByKey.get(key);
    if (existing) {
      existing.deaths += 1;
    } else {
      tallyByKey.set(key, {
        regionKey,
        factionId: event.faction_id,
        deaths: 1,
      });
    }
  }

  const factionTallies = [...tallyByKey.entries()]
    .sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0))
    .map(([, tally]) => tally);

  return {
    totalCombatDeaths: combatDeaths.length,
    regions,
    factionTallies,
  };
}

/**
 * 把 region key 翻译成匿名方位描述（无精确地名，贴"天道远方感知"基调）。
 * region cell 索引 → 大致方位（东/西/南/北 + 远近粗判）；unknown → 不知名的荒野。
 */
export function describeRegion(regionKey: string): string {
  if (regionKey === UNKNOWN_REGION_KEY) return "不知名的荒野";
  const match = /^region:(-?\d+),(-?\d+)$/.exec(regionKey);
  if (!match) return "不知名的荒野";
  const cx = Number(match[1]);
  const cz = Number(match[2]);
  // Z 负=北，正=南；X 负=西，正=东（MC 坐标系）。原点附近视为「近处」。
  const ns = cz < 0 ? "北" : cz > 0 ? "南" : "";
  const ew = cx < 0 ? "西" : cx > 0 ? "东" : "";
  const dir = `${ns}${ew}`;
  if (dir === "") return "近处";
  return `${dir}方`;
}

/** 把战报涉及的所有 region 拼成一句方位描述（去重，按首见顺序）。 */
function describeRegions(report: OffscreenWarReport): string {
  if (report.regions.length === 0) return "不知名的荒野";
  const seen = new Set<string>();
  const labels: string[] = [];
  for (const regionKey of report.regions) {
    const label = describeRegion(regionKey);
    if (!seen.has(label)) {
      seen.add(label);
      labels.push(label);
    }
  }
  return labels.join("、");
}

/**
 * 离屏战事 narration 文案——全匿名散修框架（无具名宗门，见 WORLDVIEW 锚点）。
 *
 * 3 条模板按 combat 战死总数确定性轮转（mock LLM 无文本时的 deterministic 落点），并把
 * 聚合出的方位 / 折损数填进去，让叙事跟着真实战损走而非纯静态。
 */
export function renderOffscreenWarText(report: OffscreenWarReport): string {
  const regionLabel = describeRegions(report);
  const deaths = report.totalCombatDeaths;
  const factionCount = report.factionTallies.length;
  const variant = deaths % 3;

  if (variant === 0) {
    // 模板 1（plan 行 169 具名版改写为匿名）：两方散修争脉，横尸当场，灵气复归此地。
    return `两方散修于 ${regionLabel} 争脉，${deaths} 名修士横尸当场，灵气复归此地。`;
  }
  if (variant === 1) {
    // 模板 2（plan 行 170）：某派系折损精锐，那一带灵脉反比往日丰盈。
    const sides = factionCount > 1 ? "数股散修势力" : "某派系";
    return `${regionLabel} 隐有杀伐余韵，${sides} 折损精锐，那一带灵脉竟比往日丰盈了几分。`;
  }
  // 模板 3（plan 行 171）：无名之辈的厮杀，天道不记其名，只记回流的真元。
  return `又是无名之辈的厮杀——天道不记其名，只记下 ${regionLabel} 那点回流的真元。`;
}

export function renderOffscreenWarNarration(report: OffscreenWarReport): Narration {
  return {
    scope: "broadcast",
    text: renderOffscreenWarText(report),
    style: "perception",
    kind: "scattered_cultivator",
  };
}

export interface OffscreenWarNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface OffscreenWarNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface OffscreenWarNarrationRuntimeConfig {
  sub: OffscreenWarNarrationRuntimeClient;
  pub: OffscreenWarNarrationRuntimeClient;
  logger?: OffscreenWarNarrationRuntimeLogger;
  /**
   * 推演窗口：收到首个 combat 战死后等多久再聚合 emit（让一窗内多笔战死合成一条战报）。
   * 默认 3000ms。设 0 表示来一条 combat 战死立即 flush（仍按窗口语义聚合当下 buffer）。
   */
  flushWindowMs?: number;
  /** 注入 timer（测试可传同步桩）；默认用全局 setTimeout/clearTimeout。 */
  setTimer?: (cb: () => void, ms: number) => ReturnType<typeof setTimeout>;
  clearTimer?: (handle: ReturnType<typeof setTimeout>) => void;
}

export interface OffscreenWarNarrationRuntimeStats {
  /** 收到的 bong:npc/death 事件总数（含被过滤的非 combat）。 */
  received: number;
  /** 进战报的 combat 战死总数。 */
  combatDeaths: number;
  /** 因非 combat（natural_aging 等）被忽略、未进战报的死亡数。 */
  ignoredNonCombat: number;
  /** 成功 emit 的战报 narration 条数。 */
  published: number;
  /** 校验失败（非 JSON / 契约不过 / NarrationV1 不过）的次数。 */
  rejectedContract: number;
  /** flush 时 buffer 为空（无 combat 战死）→ 不 emit 的次数。 */
  emptyFlush: number;
}

const DEFAULT_FLUSH_WINDOW_MS = 3000;

export class OffscreenWarNarrationRuntime {
  private readonly sub: OffscreenWarNarrationRuntimeClient;
  private readonly pub: OffscreenWarNarrationRuntimeClient;
  private readonly logger: OffscreenWarNarrationRuntimeLogger;
  private readonly flushWindowMs: number;
  private readonly setTimer: (cb: () => void, ms: number) => ReturnType<typeof setTimeout>;
  private readonly clearTimer: (handle: ReturnType<typeof setTimeout>) => void;
  private connected = false;
  private buffer: NpcDeathV1[] = [];
  private flushHandle: ReturnType<typeof setTimeout> | null = null;

  readonly stats: OffscreenWarNarrationRuntimeStats = {
    received: 0,
    combatDeaths: 0,
    ignoredNonCombat: 0,
    published: 0,
    rejectedContract: 0,
    emptyFlush: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== NPC_DEATH) return;
    this.handleDeathPayload(message);
  };

  constructor(config: OffscreenWarNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.flushWindowMs = config.flushWindowMs ?? DEFAULT_FLUSH_WINDOW_MS;
    this.setTimer = config.setTimer ?? ((cb, ms) => setTimeout(cb, ms));
    this.clearTimer = config.clearTimer ?? ((handle) => clearTimeout(handle));
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(NPC_DEATH);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[offscreen-war-runtime] subscribed to ${NPC_DEATH}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    if (this.flushHandle !== null) {
      this.clearTimer(this.flushHandle);
      this.flushHandle = null;
    }
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  /**
   * 处理单条 bong:npc/death wire payload：校验契约 → 过滤非 combat → 进窗口 buffer。
   * combat 战死会（按窗口策略）调度一次 flush。
   */
  handleDeathPayload(message: string): void {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[offscreen-war-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateNpcDeathV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[offscreen-war-runtime] NpcDeathV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    this.handleNpcDeath(parsed as NpcDeathV1);
  }

  /** 直接喂入一条已校验的 death 事件（测试/Context Assembler 复用入口）。 */
  handleNpcDeath(event: NpcDeathV1): void {
    this.stats.received += 1;
    if (!isOffscreenCombatDeath(event)) {
      this.stats.ignoredNonCombat += 1;
      return;
    }
    this.stats.combatDeaths += 1;
    this.buffer.push(event);
    this.scheduleFlush();
  }

  /** 喂入一批 death 事件（e2e / batch 测试入口），不调度 timer，由调用方决定何时 flush。 */
  handleNpcDeathBatch(events: readonly NpcDeathV1[]): void {
    for (const event of events) {
      this.stats.received += 1;
      if (!isOffscreenCombatDeath(event)) {
        this.stats.ignoredNonCombat += 1;
        continue;
      }
      this.stats.combatDeaths += 1;
      this.buffer.push(event);
    }
  }

  private scheduleFlush(): void {
    if (this.flushWindowMs <= 0) {
      void this.flush();
      return;
    }
    if (this.flushHandle !== null) return;
    this.flushHandle = this.setTimer(() => {
      this.flushHandle = null;
      void this.flush();
    }, this.flushWindowMs);
  }

  /**
   * 聚合当前窗口 buffer 并 emit 一条 broadcast 战事 narration。
   * 窗口内无 combat 战死（buffer 空）→ 不 emit（stats.emptyFlush++），返回 null。
   */
  async flush(): Promise<Narration | null> {
    const pending = this.buffer;
    this.buffer = [];
    if (pending.length === 0) {
      this.stats.emptyFlush += 1;
      return null;
    }

    const report = aggregateOffscreenWarReport(pending);
    const narration = renderOffscreenWarNarration(report);
    const envelope = { v: 1, narrations: [narration] };
    const validation = validateNarrationV1Contract(envelope);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[offscreen-war-runtime] NarrationV1 contract rejected:",
        validation.errors.join("; "),
      );
      return null;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.stats.published += 1;
      return narration;
    } catch (error) {
      this.logger.warn("[offscreen-war-runtime] publish failed:", error);
      return null;
    }
  }
}
