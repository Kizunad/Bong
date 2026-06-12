/**
 * plan-agent-ui-data-v1 P2/P3 — XML 模板字符串 + xmlEscape + {{param}} 插值
 *
 * 安全策略：
 * - 所有用户/世界数据插值前必须经 xmlEscape 转义（&→&amp; <→&lt; >→&gt; "→&quot; '→&apos;）
 * - null / undefined 值 → 空字符串（不抛错，安全降级）
 * - 必填参数缺失 → 抛 Error（调用方必须保证参数完整）
 * - 可选参数：整行 button 若对应 param 缺失 → 省略该 button（不渲染）
 *
 * 面板类型及 realm_gate（设计决议 §0.1）：
 *   - 活坍缩渊秘境发现  realm_gate=3（凝脉境+）
 *   - 垂死大能传承对话  realm_gate=3（凝脉境+，plan §4）
 *   - 天道启示          realm_gate=5（通灵境+）
 *
 * 模板格式：<owo-ui><components><flow-layout ...>...</flow-layout></components></owo-ui>
 * （owo-lib UIModel.load 要求 root 元素为 <owo-ui>，plan §P3 模板修正）
 *
 * P3 标准模板（plan §4）：
 *   - TSY 秘境发现：zone_name / spirit_qi_display / danger_tier / agent_narrative（均必填）
 *   - 垂死大能传承：elder_title / elder_narration / qi_cost（必填）；question_0（可选，缺失则省略 ask_question_0 按钮）
 *   - 天道启示：tiandao_message（必填）
 */

// ─── XML Escape ──────────────────────────────────────────────────────────────

/**
 * 将任意值转义为 XML 安全字符串。
 *
 * - null / undefined → ""
 * - 转义顺序：& 必须第一个（否则转义后的 & 会被再次转义）
 */
export function xmlEscape(value: unknown): string {
  if (value === null || value === undefined) {
    return "";
  }
  const str = String(value);
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

// ─── 模板插值 ────────────────────────────────────────────────────────────────

/**
 * 将 {{key}} 占位符替换为 params[key] 的 xmlEscape 值。
 *
 * @throws {Error} 如果某个 {{key}} 在 params 中不存在（必填缺失）
 */
export function interpolate(template: string, params: Record<string, unknown>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (match, key: string) => {
    if (!(key in params)) {
      throw new Error(`xmlTemplates: 必填参数缺失: "{{${key}}}" not found in params`);
    }
    const val = params[key];
    if (val === null || val === undefined) {
      return "";
    }
    return xmlEscape(val);
  });
}

// ─── 面板模板定义 ────────────────────────────────────────────────────────────

/** 面板类型枚举 */
export type UiTemplateKind =
  | "tsy_discovery"      // 活坍缩渊秘境发现（realm_gate=3）
  | "elder_legacy"       // 垂死大能传承对话（realm_gate=0）
  | "tiandao_revelation" // 天道启示（realm_gate=5）
  | "blur_insufficient"; // 境界不足模糊版（无门控限制，纯文字）

/**
 * realm_gate 按面板类型设定（设计决议 §0.1 + plan §4）：
 *   - 天道启示=5（通灵境+）, 活坍缩渊=3（凝脉境+）, 垂死传承=3（凝脉境+，plan §4）, 模糊版=0（内容已模糊，无需门控）
 */
export const TEMPLATE_REALM_GATE: Record<UiTemplateKind, number> = {
  tsy_discovery:      3,
  elder_legacy:       3,
  tiandao_revelation: 5,
  blur_insufficient:  0,
};

/**
 * 活坍缩渊秘境发现面板（realm_gate=3，凝脉境+）
 *
 * P3 标准模板（plan §4）
 * 必填参数：zone_name, spirit_qi_display, danger_tier, agent_narrative
 * allowed_button_ids: ["enter_realm", "observe_only", "dismiss"]
 */
export const TSY_DISCOVERY_TEMPLATE = `<owo-ui>
  <components>
    <flow-layout direction="vertical" gap="4">
      <label style="color:#C8A060;font=bold">【活坍缩渊】{{zone_name}}</label>
      <label style="color:#888888">残留灵压：{{spirit_qi_display}} ／ 危险等级：{{danger_tier}}</label>
      <label>{{agent_narrative}}</label>
      <flow-layout direction="horizontal" gap="8">
        <button id="enter_realm" style="color:#FFD700">踏入探寻</button>
        <button id="observe_only" style="color:#AAAAAA">神识探查（不入）</button>
        <button id="dismiss">离开</button>
      </flow-layout>
    </flow-layout>
  </components>
</owo-ui>`;

/**
 * 垂死大能传承对话面板（realm_gate=3，凝脉境+）
 *
 * P3 标准模板（plan §4）
 * 必填参数：elder_title, elder_narration, qi_cost
 * 可选参数：question_0（缺失时省略 ask_question_0 按钮）
 * allowed_button_ids: ["accept_legacy", "ask_question_0"（可选）, "dismiss"]
 *
 * 注意：qi_cost 为只读展示值，"接受传承"按钮点击后由 server 向 plan-dying-elder-v1
 * 的 legacy_accept_system 转发事件，本 plan 不发起任何 QiTransfer。
 */
export const ELDER_LEGACY_TEMPLATE_BASE = `<owo-ui>
  <components>
    <flow-layout direction="vertical" gap="4">
      <label style="color:#FF6666;font=bold">{{elder_title}} 的残余神识</label>
      <label style="color:#CCCCCC">{{elder_narration}}</label>
      <label style="color:#888888">接受传承需消耗 {{qi_cost}} 真元（由天道见证，不可撤回）</label>
      <flow-layout direction="horizontal" gap="8">
        <button id="accept_legacy">接受传承</button>
        {{OPTIONAL_QUESTION_BUTTON}}
        <button id="dismiss">离开</button>
      </flow-layout>
    </flow-layout>
  </components>
</owo-ui>`;

/** question_0 存在时插入的可选按钮片段 */
const QUESTION_BUTTON_SNIPPET = `<button id="ask_question_0">{{question_0}}</button>`;

/**
 * 渲染垂死大能传承面板，支持 question_0 可选按钮。
 *
 * @throws {Error} elder_title / elder_narration / qi_cost 必填参数缺失时
 */
export function renderElderLegacyTemplate(params: Record<string, unknown>): string {
  const hasQuestion = "question_0" in params &&
    params["question_0"] !== null &&
    params["question_0"] !== undefined;

  const questionSnippet = hasQuestion
    ? interpolate(QUESTION_BUTTON_SNIPPET, { question_0: params["question_0"] })
    : "";

  // Substitute OPTIONAL_QUESTION_BUTTON first, then interpolate required params
  const withOptional = ELDER_LEGACY_TEMPLATE_BASE.replace(
    "{{OPTIONAL_QUESTION_BUTTON}}",
    questionSnippet,
  );

  // Remove question_0 from params to avoid "missing param" error (already handled)
  const { question_0: _q, ...requiredParams } = params as Record<string, unknown>;
  return interpolate(withOptional, requiredParams);
}

/** 天道启示面板（realm_gate=5，通灵境专属）
 *
 * P3 标准模板（plan §4）
 * 必填参数：tiandao_message
 * allowed_button_ids: ["dismiss"]
 * 低境界（realm<5）玩家收到 realm_gate_rejected，Agent 降级发 narration（设计决议 §8 Q3）
 */
export const TIANDAO_REVELATION_TEMPLATE = `<owo-ui>
  <components>
    <flow-layout direction="vertical" gap="6">
      <label style="color:#8888FF;font=bold">天意</label>
      <label style="color:#CCCCCC">{{tiandao_message}}</label>
      <label style="color:#666666;font=italic">（此感应无需回应——天道不等你）</label>
      <button id="dismiss" style="color:#AAAAAA">闭目冥思</button>
    </flow-layout>
  </components>
</owo-ui>`;

/**
 * 境界不足模糊版面板（纯文字，realm < realm_gate 时显示）
 *
 * 必填参数：blur_hint（模糊感应的描述文字）
 * realm_gate=0（此版本已模糊处理，服务端不需要再门控）
 */
export const BLUR_INSUFFICIENT_TEMPLATE = `<owo-ui>
  <components>
    <flow-layout direction="vertical" gap="4">
      <label style="color:#888888;italic">有什么正在靠近</label>
      <label style="color:#666666">{{blur_hint}}</label>
      <button id="dismiss" style="color:#666666">散去</button>
    </flow-layout>
  </components>
</owo-ui>`;

const TEMPLATE_MAP: Record<Exclude<UiTemplateKind, "elder_legacy">, string> = {
  tsy_discovery:      TSY_DISCOVERY_TEMPLATE,
  tiandao_revelation: TIANDAO_REVELATION_TEMPLATE,
  blur_insufficient:  BLUR_INSUFFICIENT_TEMPLATE,
};

// ─── 渲染入口 ────────────────────────────────────────────────────────────────

export interface RenderTemplateOptions {
  kind: UiTemplateKind;
  params: Record<string, unknown>;
}

/**
 * 渲染面板 XML 字符串。
 *
 * - elder_legacy 支持 question_0 可选按钮（缺失则省略 ask_question_0）
 * - 其余面板使用简单 {{param}} 插值（必填缺失抛 Error）
 *
 * @throws {Error} 如果 params 缺少必填参数
 */
export function renderTemplate(options: RenderTemplateOptions): string {
  const { kind, params } = options;
  if (kind === "elder_legacy") {
    return renderElderLegacyTemplate(params);
  }
  const template = TEMPLATE_MAP[kind];
  return interpolate(template, params);
}

// ─── realm rank 映射（与 server Realm::rank() 完全一致） ─────────────────────

/**
 * 世界状态中 player.realm 字符串 → 1-indexed rank（与 server `Realm::rank()` 完全一致）。
 *
 * 醒灵=1, 引气=2, 凝脉=3, 固元=4, 通灵=5, 化虚=6
 * 未知/空字符串 → 0（视为未达任何境界，realm_gate>0 时必然被拒绝）
 */
// ─── XML button ID 提取 ───────────────────────────────────────────────────────

/**
 * 从 XML 字符串中提取所有 button id 属性（最多 16 个）。
 * 同时支持单引号和双引号属性值。
 */
export function extractButtonIds(xml: string): string[] {
  const ids: string[] = [];
  const re = /<button[^>]+id=["']([^"']+)["'][^>]*>/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(xml)) !== null && ids.length < 16) {
    ids.push(m[1]);
  }
  return ids;
}

// ─── realm rank 映射（与 server Realm::rank() 完全一致） ─────────────────────

export function realmStringToRank(realm: string | undefined | null): number {
  switch (realm) {
    case "Awaken":    return 1; // 醒灵
    case "Induce":    return 2; // 引气
    case "Condense":  return 3; // 凝脉
    case "Solidify":  return 4; // 固元
    case "Spirit":    return 5; // 通灵
    case "Void":      return 6; // 化虚
    default:          return 0;
  }
}
