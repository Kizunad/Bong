import {
  CHANNELS,
  type Narration,
  type YidaoEventV1,
  validateNarrationV1Contract,
  validateYidaoEventV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, YIDAO_EVENT } = CHANNELS;

export interface YidaoNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface YidaoNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface YidaoNarrationRuntimeStats {
  received: number;
  ignored: number;
  published: number;
  rejectedContract: number;
}

export interface YidaoNarrationRuntimeConfig {
  sub: YidaoNarrationRuntimeClient;
  pub: YidaoNarrationRuntimeClient;
  logger?: YidaoNarrationRuntimeLogger;
}

export function renderYidaoNarration(event: YidaoEventV1): Narration | null {
  const medic = shortName(event.medic_id);
  const patient = event.patient_ids.length === 1
    ? shortName(event.patient_ids[0])
    : `${event.patient_ids.length} 名患者`;
  const target = `yidao:${event.kind}|medic:${event.medic_id}|tick:${event.tick}`;

  let text: string;
  switch (event.kind) {
    case "meridian_heal":
      text = event.success_count > 0
        ? `${medic} 以平和真元替 ${patient} 接回${meridianText(event.meridian_id)}，断处真元一寸寸收束，没有外放成杀意。`
        : `${medic} 的接经术在 ${patient} 脉口前散开，断脉未合，业力在针尾沉了一点。`;
      break;
    case "contam_purge":
      text = `${medic} 替 ${patient} 引开异种真元，污染削去 ${formatNumber(event.contam_reduced)}，余下的浊气被压回经络边缘。`;
      break;
    case "emergency_resuscitate":
      text = `${medic} 俯身急救 ${patient}，止住出血并回暖 ${formatNumber(event.hp_restored)} 点气血，命线暂时稳住。`;
      break;
    case "life_extension":
      text = `${medic} 为 ${patient} 施续命术，真元逆灌换回一口气；业力 +${formatNumber(event.karma_delta)}，医患双方真元上限都留下缺口。`;
      break;
    case "mass_heal":
      text = `${medic} 展开群体接经场，同时处理 ${event.success_count} 条断脉；每名患者仍是独立手术，代价按人头落在医者身上。`;
      break;
    case "karma_accumulation":
      text = `${medic} 的医道业力再增 ${formatNumber(event.karma_delta)}，救人留下的因果已经能被天道记账。`;
      break;
    case "medical_contract":
      text = `${medic} 与 ${patient} 的医患关系写入长期账册，状态转为 ${contractStateText(event.contract_state)}。`;
      break;
    default:
      return null;
  }

  if (event.contract_state && event.kind !== "medical_contract") {
    text += ` 医患关系转为 ${contractStateText(event.contract_state)}。`;
  }

  const narration: Narration = {
    scope: "player",
    target,
    text,
    style: event.kind === "life_extension" || event.kind === "karma_accumulation"
      ? "system_warning"
      : "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export class YidaoNarrationRuntime {
  private readonly sub: YidaoNarrationRuntimeClient;
  private readonly pub: YidaoNarrationRuntimeClient;
  private readonly logger: YidaoNarrationRuntimeLogger;
  private connected = false;

  readonly stats: YidaoNarrationRuntimeStats = {
    received: 0,
    ignored: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== YIDAO_EVENT) return;
    void this.handlePayload(message);
  };

  constructor(config: YidaoNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(YIDAO_EVENT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[yidao-runtime] subscribed to ${YIDAO_EVENT}`);
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
      this.logger.warn("[yidao-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateYidaoEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[yidao-runtime] invalid yidao event:", validation.errors.join("; "));
      return;
    }
    this.stats.received += 1;

    const narration = renderYidaoNarration(parsed as YidaoEventV1);
    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[yidao-runtime] publish failed:", error);
    }
  }
}

function meridianText(meridianId: string | undefined): string {
  return meridianId ? ` ${meridianId} 经` : "断脉";
}

function contractStateText(state: string | undefined): string {
  switch (state) {
    case "patient":
      return "患者";
    case "long_term_patient":
      return "长期患者";
    case "bonded":
      return "结契";
    case "stranger":
      return "陌生人";
    default:
      return "未定";
  }
}

function formatNumber(value: number): string {
  return Number.isFinite(value)
    ? new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 }).format(value)
    : "0";
}

function shortName(id: string): string {
  const stripped = id
    .replace(/^offline:/u, "")
    .replace(/^entity_bits:/u, "实体");
  return stripped.length > 0 ? stripped : "某修士";
}
