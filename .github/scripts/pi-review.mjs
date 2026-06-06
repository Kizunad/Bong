#!/usr/bin/env node
// Pi Plan-Aware Reviewer —— 确定性轮控 + 自信度门控 + 低自信精准 fan-out。
// 触发:在 PR 评论 `/review pi`(不用 @pi——那会 mention 到 GitHub 上的真实用户 github.com/pi)。
//
// 设计要点(为何不再用 pi-coding-agent-action / hive-think 扩展):
//   旧版 Pi 是一个【无界 agent 循环】——pi -p 没有轮数上限,只在模型自己停手 /
//   命中 30min 硬超时时退出,所以一次 review 能烧 30+ 分钟。本脚本把"轮"从 agent
//   内部搬到 orchestrator 外部:每一"轮" = 一次【单轮、无工具】的模型调用(天生有界),
//   轮与轮之间由本脚本(确定性代码)控制——这才是真的硬上限,模型给不了。
//
// 流程(分层 + 轮控):
//   第零步 gather(flash,1 次):把 diff + plan 压成紧凑 brief(逐文件摘要 + plan 交付物 checklist)。
//   第一步 debate 轮控(pro 档面板,最多 MAX_ROUNDS 轮):
//     - 每轮 N 个面板成员并行,各自基于 brief + 累积上下文给 {confidence, findings, uncertain_dimensions, files_needed}
//     - 轮间注入【倒计时 + 语气】:从容核查 → 还剩 N 轮 → ⚠️只剩 1 轮 → ⛔最终轮硬上限
//     - 面板自信度中位数 ≥ 目标 → 提前收敛;否则把成员请求的文件抓进上下文,开下一轮
//     - 到 MAX_ROUNDS 强制收口(标注未收敛)
//   低自信 fan-out:轮控结束仍 < 目标且有 uncertain_dimensions → 每个弱维派一个【专项】成员并行深挖。
//   第二步 arbiter:综合全池 findings(去重 + 共识计数)产出中文结构化 review,发评论。
//
// 自包含:只用 Node 内置 fetch + gh CLI,无 npm 依赖。端点/模型/key/轮控参数全 env 可覆盖。
// 纯逻辑(computeCountdown / aggregateConfidence / dedupeFindings / extractJSON / normalizeConfidence)
// 导出供 pi-review.test.mjs 用 `node --test` 锁行为;主流程仅在作为入口时执行。

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";

// ── 配置(全部 env 可覆盖)────────────────────────────────────────────────────
// 多 provider 路由:模型名用 `provider/id`(如 cliproxy/deepseek-v4-flash、deepseek/deepseek-v4-pro、
// kiro/claude-sonnet-4-6)。baseUrl 写死已知端点(可 env 覆盖),key 从 per-provider secret 注入,
// api 区分 OpenAI(/chat/completions)与 Anthropic(/v1/messages)两种格式。这样"力大砖飞"才能在
// 一次 review 里同时混免费队(cliproxy/kiro/ollama)+ 付费 pro(deepseek)。
const PROVIDERS = {
  cliproxy: { baseUrl: process.env.PI_CLIPROXY_BASE_URL || "https://proxy.kizun4.uk/v1", key: process.env.PI_CLIPROXY_KEY, api: "openai" },
  deepseek: { baseUrl: process.env.PI_DEEPSEEK_BASE_URL || "https://api.deepseek.com", key: process.env.PI_DEEPSEEK_KEY, api: "openai" },
  ollama: { baseUrl: process.env.PI_OLLAMA_BASE_URL || "https://ollama.com/v1", key: process.env.PI_OLLAMA_KEY, api: "openai" },
  kiro: { baseUrl: process.env.PI_KIRO_BASE_URL || "https://kiro.kizunadesu.cc", key: process.env.PI_KIRO_KEY, api: "anthropic" },
  anyrouter: { baseUrl: process.env.PI_ANYROUTER_BASE_URL || "https://anyrouter.top/v1", key: process.env.PI_ANYROUTER_KEY, api: "openai" },
};
const DEFAULT_PROVIDER = process.env.PI_DEFAULT_PROVIDER || "cliproxy";
const ANTHROPIC_MAX_TOKENS = Math.max(1024, parseInt(process.env.PI_ANTHROPIC_MAX_TOKENS || "8000", 10));
const RETRIES = Math.max(1, parseInt(process.env.PI_RETRIES || "4", 10));
const usageByModel = {};

// 分层角色模型(provider 限定名,env 可覆盖):动态分档按这三档角色拼面板。
const FLASH_MODEL = process.env.PI_FLASH_MODEL || "cliproxy/deepseek-v4-flash"; // 廉价免费,主力
const PRO_MODEL = process.env.PI_PRO_MODEL || "deepseek/deepseek-v4-pro"; // 付费深推理
const LITE_MODEL = process.env.PI_LITE_MODEL || "cliproxy/sensenova-6.7-flash-lite"; // 免费,异源多样性
const GATHER_MODEL = process.env.PI_GATHER_MODEL || FLASH_MODEL; // gather 永远用廉价 flash

// 动态分档(autoscale 默认开):main() 取到 diff 规模后按 TIERS/pickTier 选面板配比 + 轮数 + arbiter 档。
// 显式设 PI_DEBATE_MODELS / PI_MAX_ROUNDS / PI_PANEL / PI_ARBITER_MODEL → 钉死该维度,跳过该维 autoscale。
const AUTOSCALE = process.env.PI_AUTOSCALE !== "0";
const DEBATE_MODELS_PIN = process.env.PI_DEBATE_MODELS ? expandModelList(process.env.PI_DEBATE_MODELS) : null;
const MAX_ROUNDS_PIN = process.env.PI_MAX_ROUNDS ? Math.max(1, parseInt(process.env.PI_MAX_ROUNDS, 10)) : null;
const PANEL_PIN = process.env.PI_PANEL ? Math.max(1, parseInt(process.env.PI_PANEL, 10)) : null;
const ARBITER_PIN = process.env.PI_ARBITER_MODEL || null;
// 以下四项在 main() 里按档/按 pin 最终敲定(故为 let);此处给安全默认,避免 chat 默认参数取到 undefined。
let DEBATE_MODELS = DEBATE_MODELS_PIN || expandModelList(`${FLASH_MODEL}*3`);
let MAX_ROUNDS = MAX_ROUNDS_PIN || 2;
let PANEL = PANEL_PIN || DEBATE_MODELS.length;
let ARBITER_MODEL = ARBITER_PIN || PRO_MODEL;
let TIER_LABEL = "default";
const CONFIDENCE_TARGET = clampPct(parseFloat(process.env.PI_CONFIDENCE_TARGET || "80"), 80); // 中位数 ≥ 此值即收敛
const CONCURRENCY = Math.max(1, parseInt(process.env.PI_CONCURRENCY || "6", 10));
const FANOUT_ENABLED = process.env.PI_FANOUT !== "0"; // 低自信精准 fan-out 开关
const FANOUT_MAX = Math.max(1, parseInt(process.env.PI_FANOUT_MAX || "4", 10)); // 最多派几个专项成员
const CALL_TIMEOUT_MS = Math.max(30_000, parseInt(process.env.PI_CALL_TIMEOUT_MS || "300000", 10)); // 单次调用硬超时
const MAX_FILES = Math.max(0, parseInt(process.env.PI_MAX_FILES || "6", 10)); // 每轮最多抓几个补充文件
const FILE_BYTES = Math.max(1000, parseInt(process.env.PI_FILE_BYTES || "8000", 10)); // 每个补充文件取多少字符
const MAX_DIFF = parseInt(process.env.PI_MAX_DIFF || "200000", 10);

const PR = process.env.PR_NUMBER;

// ── 纯逻辑(导出供测试)────────────────────────────────────────────────────────

// 把任意值收成 [0,100] 的整数百分比;非法返回 fallback(默认 null = 无效,不计入聚合)。
export function normalizeConfidence(v, fallback = null) {
  const n = typeof v === "string" ? parseFloat(v) : v;
  if (typeof n !== "number" || !Number.isFinite(n)) return fallback;
  return Math.max(0, Math.min(100, Math.round(n)));
}

function clampPct(v, fallback) {
  const n = normalizeConfidence(v);
  return n === null ? fallback : n;
}

// 面板自信度聚合:取中位数做收敛门控(避免单个离群值左右大局),同时给 min/max/n。
export function aggregateConfidence(values) {
  const nums = (values || []).map((v) => normalizeConfidence(v)).filter((n) => n !== null);
  if (nums.length === 0) return { median: null, min: null, max: null, n: 0 };
  const sorted = nums.slice().sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  const median = sorted.length % 2 ? sorted[mid] : Math.round((sorted[mid - 1] + sorted[mid]) / 2);
  return { median, min: sorted[0], max: sorted[sorted.length - 1], n: sorted.length };
}

// 倒计时 + 语气:随剩余轮数逐级加码,最后一轮硬收口。round/maxRounds 均 1-indexed。
export function computeCountdown(round, maxRounds) {
  const left = maxRounds - round;
  if (maxRounds <= 1) {
    return {
      left: 0,
      label: "唯一一轮 · 硬上限",
      text: "【唯一一轮 · ⛔ 硬上限】只有这一轮,直接给出最终 findings 与最终 confidence,不要留待下一轮。",
    };
  }
  if (round <= 1) {
    return {
      left,
      label: `第 1/${maxRounds} 轮 · 从容核查`,
      text:
        `【第 1/${maxRounds} 轮 · 从容核查】本轮全面排查,不急于下结论。把还没把握的点明确写进 ` +
        `uncertain_dimensions,把还想看的代码写进 files_needed——后续轮会据此为你补充上下文。`,
    };
  }
  if (left >= 2) {
    return {
      left,
      label: `第 ${round}/${maxRounds} 轮 · 还剩 ${left} 轮`,
      text:
        `【第 ${round}/${maxRounds} 轮 · 还剩 ${left} 轮】已按你上一轮的 files_needed 补充了上下文(见下)。` +
        `继续核查,优先攻克上一轮标记的弱项;仍不确定的继续写进 uncertain_dimensions / files_needed。`,
    };
  }
  if (left === 1) {
    return {
      left,
      label: `第 ${round}/${maxRounds} 轮 · ⚠️ 只剩 1 轮`,
      text:
        `【第 ${round}/${maxRounds} 轮 · ⚠️ 只剩 1 轮】下一轮就是硬上限。集中火力把最关键、最有把握的问题钉死,` +
        `拿不准的明确标注,不要再铺新摊子。`,
    };
  }
  return {
    left: 0,
    label: `第 ${maxRounds}/${maxRounds} 轮 · ⛔ 最终轮`,
    text:
      `【第 ${maxRounds}/${maxRounds} 轮 · ⛔ 最终轮 · 硬上限】这是最后一轮,之后强制收口。必须给出最终 findings 与` +
      `最终 confidence——即便不完全确定,也要基于现有证据做出最佳判断,并把残余不确定项明确标注。`,
  };
}

const SEV_RANK = { blocker: 0, major: 1, minor: 2 };
const sevRank = (s) => (SEV_RANK[String(s || "").toLowerCase()] ?? 3);

// 跨轮/跨成员去重:同 file|line|标题(归一化)合并,取最高严重度,累计 consensus(几个成员独立提出)。
export function dedupeFindings(findings) {
  const byKey = new Map();
  for (const f of findings || []) {
    if (!f || (!f.title && !f.file)) continue;
    const file = String(f.file || "").trim();
    const line = String(f.line ?? "").trim();
    const title = String(f.title || "").trim();
    const key = `${file}|${line}|${title.toLowerCase().replace(/\s+/g, " ")}`;
    const existing = byKey.get(key);
    if (!existing) {
      byKey.set(key, {
        severity: String(f.severity || "minor").toLowerCase(),
        file,
        line,
        title,
        evidence: String(f.evidence || ""),
        why: String(f.why || ""),
        consensus: 1,
        sources: [f.by].filter(Boolean),
      });
      continue;
    }
    existing.consensus += 1;
    if (f.by) existing.sources.push(f.by);
    if (sevRank(f.severity) < sevRank(existing.severity)) existing.severity = String(f.severity).toLowerCase();
    if (String(f.evidence || "").length > existing.evidence.length) existing.evidence = String(f.evidence);
    if (String(f.why || "").length > existing.why.length) existing.why = String(f.why);
  }
  return [...byKey.values()].sort(
    (a, b) => sevRank(a.severity) - sevRank(b.severity) || b.consensus - a.consensus,
  );
}

// 从模型输出抠第一段完整 JSON(容忍 ```json 围栏 + 前后废话),balanced-bracket 扫描。
export function extractJSON(text) {
  if (!text) return null;
  let t = String(text).trim();
  const fence = t.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fence) t = fence[1].trim();
  try {
    return JSON.parse(t);
  } catch {
    /* fall through */
  }
  const open = t.search(/[[{]/);
  if (open < 0) return null;
  const stack = [];
  let inStr = false;
  let esc = false;
  for (let i = open; i < t.length; i++) {
    const c = t[i];
    if (inStr) {
      if (esc) esc = false;
      else if (c === "\\") esc = true;
      else if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') inStr = true;
    else if (c === "[" || c === "{") stack.push(c);
    else if (c === "]" || c === "}") {
      stack.pop();
      if (stack.length === 0) {
        try {
          return JSON.parse(t.slice(open, i + 1));
        } catch {
          return null;
        }
      }
    }
  }
  return null;
}

// 把 debate/fanout 成员的原始输出解析成统一结构(缺字段给安全默认)。
export function parseMember(raw) {
  const j = extractJSON(raw) || {};
  const findings = Array.isArray(j.findings) ? j.findings : Array.isArray(j) ? j : [];
  return {
    confidence: normalizeConfidence(j.confidence),
    findings: findings.filter((f) => f && (f.title || f.file)),
    uncertain: (Array.isArray(j.uncertain_dimensions) ? j.uncertain_dimensions : [])
      .map((u) => (typeof u === "string" ? { dimension: u } : u))
      .filter((u) => u && u.dimension),
    filesNeeded: (Array.isArray(j.files_needed) ? j.files_needed : [])
      .map((p) => String(p || "").trim())
      .filter(Boolean),
  };
}

// 合法的"待抓取"文件路径:相对仓库根、无 .. 穿越、无绝对路径。
export function isSafeRepoPath(p) {
  const s = String(p || "").trim();
  if (!s || s.startsWith("/") || s.includes("..") || s.includes("\0")) return false;
  return /^[\w./-]+$/.test(s);
}

// 展开 `name*count` 语法:`"a/x*3, b/y, c/z*2"` → ["a/x","a/x","a/x","b/y","c/z","c/z"]。
// 这是"力大砖飞"的人体工学入口——`flash*20` 比手敲 20 遍清爽,PANEL 默认即取展开后长度。
export function expandModelList(spec) {
  return String(spec || "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .flatMap((item) => {
      const m = item.match(/^(.+?)\s*\*\s*(\d+)$/);
      if (m) return Array.from({ length: Math.max(1, parseInt(m[2], 10)) }, () => m[1].trim());
      return [item];
    });
}

// 拆 `provider/id`:`"cliproxy/deepseek-v4-flash"` → {provider:"cliproxy", id:"deepseek-v4-flash"}。
// 无前缀 → {provider:null, id} (调用方用 DEFAULT_PROVIDER 兜底)。只在首个 "/" 处切分。
export function parseModel(full) {
  const s = String(full || "").trim();
  const i = s.indexOf("/");
  if (i < 0) return { provider: null, id: s };
  return { provider: s.slice(0, i), id: s.slice(i + 1) };
}

// 动态分档表(均衡档):按总变更行数选档,定 flash/pro/lite 数量 + 轮数。小改省钱省时,大改力大砖飞。
// pro>0 的档 arbiter 用付费 pro,trivial(pro=0)全程零付费。maxLines 升序、最后一档 Infinity 兜底。
export const TIERS = [
  { label: "trivial", maxLines: 40, flash: 4, pro: 0, lite: 2, rounds: 1 },
  { label: "small", maxLines: 250, flash: 8, pro: 1, lite: 3, rounds: 1 },
  { label: "medium", maxLines: 1000, flash: 14, pro: 2, lite: 5, rounds: 2 },
  { label: "large", maxLines: Infinity, flash: 20, pro: 4, lite: 8, rounds: 2 },
];

// 选档:总变更行数定基础档;文件数 ≥ bumpFiles(改动面广)升一档,封顶最后一档。
export function pickTier(changedLines, changedFiles, { tiers = TIERS, bumpFiles = 15 } = {}) {
  const lines = Math.max(0, Number(changedLines) || 0);
  let idx = tiers.findIndex((t) => lines <= t.maxLines);
  if (idx < 0) idx = tiers.length - 1;
  if ((Number(changedFiles) || 0) >= bumpFiles && idx < tiers.length - 1) idx += 1;
  return tiers[idx];
}

// 把一档的 flash/pro/lite 数量按角色模型名拼成展开后的面板清单(数量为 0 的角色略过)。
export function buildTierPanel(tier, { flashModel, proModel, liteModel }) {
  const parts = [];
  if (tier.flash > 0) parts.push(`${flashModel}*${tier.flash}`);
  if (tier.pro > 0) parts.push(`${proModel}*${tier.pro}`);
  if (tier.lite > 0) parts.push(`${liteModel}*${tier.lite}`);
  return expandModelList(parts.join(","));
}

// ── 项目审核准则(精简版,静态 worldview slice,塞进每个成员上下文)────────────────
const GUIDELINES = `
## Bong 项目审核准则(末法残土修仙沙盒,三层架构 server/Rust + agent/TS + client/Java)

### 世界观锚定(docs/worldview.md 是正典,代码不得矛盾)
- 六境界:醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚。禁用"筑基/金丹/元婴"等传统称谓。
- 货币:骨币(通货)、灵石(燃料,非货币)。不得出现"灵石=钱"的逻辑。
- 灵气守恒律:全服灵气总量恒定。真元流动必须走 qi_physics::QiTransfer{from,to,amount}。红旗:
  qi_current += X 无对应 zone 减 / zone.spirit_qi -= Y 无对应玩家增 / 衰变让真元凭空消失 / 招式只扣攻方不写环境。
- 物理常数唯一源:衰减/逸散/半衰常数必须来自 qi_physics,禁止各 plan 硬编 *_DECAY*/*_DRAIN*/0.0X_f64。
- 命名"末法去上古",不要仙气飘飘。

### 架构硬约束
- 跨层通信只走 Redis IPC + CustomPayload。
- IPC schema:TypeBox(TS)是 source of truth → JSON Schema → Rust serde,改 schema 必须双端同步。
- 新增 SkillRegistry::register 必须同步在 cultivation::meridian::severed::SkillMeridianDependencies::declare 注册依赖经脉。
- Bevy ECS:component 是数据、system 是逻辑,别在 component 写逻辑方法。

### 代码与测试
- 简单易懂 > 花里胡哨;过度抽象/无用注释/超出 plan 的功能蔓延都是减分项。
- 测契约不测实现;测试要饱和:happy path + 所有边界 + 所有错误分支 + 所有状态转换。
`.trim();

// ── IO 工具 ──────────────────────────────────────────────────────────────────
function gh(args) {
  return execSync(`gh ${args}`, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function mapLimit(items, limit, fn) {
  const out = new Array(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      out[i] = await fn(items[i], i);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return out;
}

// 单次 chat:无工具单轮调用(天生有界),按 model 的 `provider/id` 路由到对应端点/key/API 格式,带超时 + 重试退避。
// 支持 OpenAI(/chat/completions)与 Anthropic(/v1/messages)两种格式;某 provider 无 key 直接跳过(返回 null,
// 上层 graceful-degrade,死模型不阻断全局)。
async function chat(content, { label = "", model = DEBATE_MODELS[0], retries = RETRIES } = {}) {
  const { provider, id } = parseModel(model);
  const prov = PROVIDERS[provider || DEFAULT_PROVIDER];
  if (!prov || !prov.key) {
    console.error(`[${label}] ${model}: provider「${provider || DEFAULT_PROVIDER}」无配置或无 key,跳过`);
    return null;
  }
  const base = prov.baseUrl.replace(/\/+$/, "");
  for (let attempt = 1; attempt <= retries; attempt++) {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), CALL_TIMEOUT_MS);
    try {
      let url, headers, body;
      if (prov.api === "anthropic") {
        url = `${base}/v1/messages`;
        headers = { "Content-Type": "application/json", "x-api-key": prov.key, "anthropic-version": "2023-06-01" };
        body = JSON.stringify({ model: id, max_tokens: ANTHROPIC_MAX_TOKENS, messages: [{ role: "user", content }] });
      } else {
        url = `${base}/chat/completions`;
        headers = { "Content-Type": "application/json", Authorization: `Bearer ${prov.key}` };
        body = JSON.stringify({ model: id, messages: [{ role: "user", content }] });
      }
      const res = await fetch(url, { method: "POST", headers, body, signal: ctrl.signal });
      clearTimeout(timer);
      if (!res.ok) {
        const b = (await res.text()).slice(0, 300);
        throw new Error(`HTTP ${res.status} ${b}`);
      }
      const data = await res.json();
      let text, promptTok, completionTok;
      if (prov.api === "anthropic") {
        text = (data?.content || [])
          .filter((c) => c?.type === "text")
          .map((c) => c.text)
          .join("");
        promptTok = data?.usage?.input_tokens || 0;
        completionTok = data?.usage?.output_tokens || 0;
      } else {
        text = data?.choices?.[0]?.message?.content;
        promptTok = data?.usage?.prompt_tokens || 0;
        completionTok = data?.usage?.completion_tokens || 0;
      }
      if (promptTok || completionTok) {
        const m = usageByModel[model] || (usageByModel[model] = { prompt: 0, completion: 0, total: 0, calls: 0 });
        m.prompt += promptTok;
        m.completion += completionTok;
        m.total += promptTok + completionTok;
        m.calls += 1;
      }
      if (!text || !text.trim()) throw new Error("空响应");
      return text;
    } catch (e) {
      clearTimeout(timer);
      console.error(`[${label}] ${model} 第 ${attempt}/${retries} 次失败: ${e.message}`);
      if (attempt === retries) return null;
      await sleep(2000 * attempt);
    }
  }
  return null;
}

// 抓取成员请求的补充文件(本地 main 签出版本,带 caveat)。去重 + 上限 + 截断 + 路径安全过滤。
export function fetchFiles(paths, already) {
  const picked = [];
  for (const p of paths) {
    if (picked.length >= MAX_FILES) break;
    if (already.has(p) || !isSafeRepoPath(p) || !existsSync(p)) continue;
    already.add(p);
    let body = readFileSync(p, "utf8");
    const truncated = body.length > FILE_BYTES;
    if (truncated) body = body.slice(0, FILE_BYTES);
    picked.push(`### ${p}${truncated ? `(前 ${FILE_BYTES} 字符)` : ""}\n\`\`\`\n${body}\n\`\`\``);
  }
  if (!picked.length) return "";
  return (
    `\n## 补充代码上下文(main 分支版本——PR 若改动了这些文件,以上面 diff 为准,此处仅供周边代码参考)\n` +
    picked.join("\n\n")
  );
}

// ── 提示词 ───────────────────────────────────────────────────────────────────
const gatherPrompt = (prContext) =>
  `你是 Bong PR 审核的资料整理 scout(廉价档)。**这是【整理】任务,不是决策——不要给评分/建议/结论。**\n` +
  `${GUIDELINES}\n\n${prContext}\n\n` +
  `产出一份紧凑 brief,只输出纯 JSON(不要解释文字):\n` +
  `{\n` +
  `  "diff_summary": [{"file":"路径","change":"这个文件改了什么(一句话)","anchors":["reviewer 必看的 file:line"]}],\n` +
  `  "plan_checklist": [{"item":"plan 该阶段承诺的交付物(模块/函数/测试/schema/跨仓库 symbol)","where":"应落在哪个文件"}],\n` +
  `  "worldview_topics": ["本 diff 实际涉及的世界观主题(境界命名/货币/灵气守恒/物理常数/招式经脉/IPC schema/命名风格),不涉及就空数组"]\n` +
  `}\n` +
  `facts only;plan 从 PR 标题/分支名识别 plan-<name>-vN,识别不到 plan_checklist 给空数组。`;

const debatePrompt = (brief, extraContext, countdown, priorUncertain) =>
  `你是 Bong 项目 PR 审核 hive 的一员(pro 档,深度推理,独立判断)。\n${GUIDELINES}\n\n` +
  `## 已整理的审核 brief\n${brief}\n` +
  (priorUncertain.length
    ? `\n## 上一轮面板标记的弱项(请优先攻克)\n${priorUncertain.map((u) => `- ${u.dimension}: ${u.reason || ""} ${u.need ? `(需:${u.need})` : ""}`).join("\n")}\n`
    : "") +
  `${extraContext}\n\n` +
  `${countdown.text}\n\n` +
  `基于以上材料审核本 PR,逐项核对:\n` +
  `a) plan 该阶段交付物是否逐项落地(对照 plan_checklist 标 ✅/❌/⚠️)\n` +
  `b) plan 列出但 PR 缺失的模块/函数/测试/schema\n` +
  `c) 正确性:逻辑、边界 off-by-one、并发、错误分支、IPC schema 对齐\n` +
  `d) 世界观对齐:六境界命名 / 骨币灵石 / 末法风格\n` +
  `e) 灵气守恒:真元增减是否走 qi_physics::QiTransfer、物理常数来源是否唯一\n` +
  `f) 测试饱和:happy + 边界 + 错误分支 + 状态转换\n` +
  `g) 简洁度:过度抽象 / 冗余注释 / 超出 plan 的功能蔓延\n\n` +
  `每条 finding 必须带 file:line + diff 证据片段。**没把握 / 推测性的不要当 finding 报**,而是写进 ` +
  `uncertain_dimensions 说明你还差什么、把想看的文件写进 files_needed(下一轮会补给你)。没问题就空数组,严禁凑数。\n\n` +
  `只输出纯 JSON(不要解释文字):\n` +
  `{\n` +
  `  "confidence": 0-100,  // 你对"本轮审核已充分、结论可靠"的自信度;材料不足就给低分\n` +
  `  "findings": [{"severity":"blocker|major|minor","file":"路径","line":"行号/范围","title":"一句话问题","evidence":"diff 中具体代码","why":"为什么是问题"}],\n` +
  `  "uncertain_dimensions": [{"dimension":"a-g 之一或具体主题","reason":"为何还没把握","need":"还需要看什么才能定论"}],\n` +
  `  "files_needed": ["还想查看的仓库文件路径(相对仓库根)"]\n` +
  `}`;

const fanoutPrompt = (brief, extraContext, dim) =>
  `你是 Bong PR 审核的【${dim.dimension}】专项核查员。前几轮综合审核对该维度把握不足,现在只盯这一维度深挖到底。\n` +
  `${GUIDELINES}\n\n## 审核 brief\n${brief}\n${extraContext}\n\n` +
  `## 待澄清\n${dim.reason || ""}${dim.need ? `(还需:${dim.need})` : ""}\n\n` +
  `只就【${dim.dimension}】给结论:有问题逐条带 file:line + 证据;确认无问题就明说"该维度核查通过"。\n` +
  `只输出纯 JSON:\n` +
  `{"dimension":"${dim.dimension}","confidence":0-100,"verdict":"问题|通过|仍不确定",` +
  `"findings":[{"severity":"blocker|major|minor","file":"路径","line":"行号","title":"...","evidence":"...","why":"..."}]}`;

const arbiterPrompt = (brief, deduped, status, planName) =>
  `你是本次 Pi 审核的**总裁判**(综合 + 发布,从严,宁缺勿滥)。\n${GUIDELINES}\n\n` +
  `## 审核 brief\n${brief}\n\n` +
  `## 经轮控 + 自信度门控存活的 findings(已去重,consensus = 几个成员独立提出)\n` +
  `${deduped.map((f) => `- [${f.severity}] ${f.file}:${f.line} — ${f.title}${f.why ? `(${f.why})` : ""} 〔consensus ${f.consensus}〕`).join("\n") || "(无)"}\n\n` +
  `## 收敛状态\n${status}\n\n` +
  `据此产出**最终中文 PR review(markdown)**。硬要求:\n` +
  `- **只就上面这些 findings 写**,严禁自行新增未列出的问题,严禁凑数。明显不成立的可直接否决。\n` +
  `- 每条带 file:line 与证据。\n` +
  `- 结构(无内容的小节可省略):\n` +
  `  **📋 Plan 对齐度**${planName ? `(${planName})` : ""} —— 交付物逐项 ✅/❌/⚠️ + 整体评级\n` +
  `  **🌍 世界观合规** —— 境界命名/货币/灵气守恒;无问题写"✅ 未发现世界观偏差"\n` +
  `  **🐛 Bug 与正确性** · **📐 代码质量** · **⚠️ 缺失与风险** · **💡 改进建议(非阻塞)**\n` +
  `- 没有高置信度问题就只写一条简短总结 + "未发现阻塞问题",不要硬找。\n` +
  `- 直接给 markdown 正文,不要输出 JSON、不要复述本提示词。`;

// ── 主流程 ───────────────────────────────────────────────────────────────────
async function main() {
  // PR 是唯一被插进 gh shell 命令的外部值——强制数字,杜绝命令注入(纵深防御;它本来源自 github.event.issue.number)。
  if (!PR || !/^\d+$/.test(String(PR))) {
    console.error("PR_NUMBER 未设置或非法(必须是纯数字)");
    process.exit(1);
  }
  const liveProviders = Object.entries(PROVIDERS).filter(([, p]) => p.key).map(([n]) => n);
  if (liveProviders.length === 0) {
    console.error("没有可用 provider key(在 repo secrets 配 PI_CLIPROXY_KEY / PI_DEEPSEEK_KEY / PI_KIRO_KEY / PI_OLLAMA_KEY 等)");
    process.exit(1);
  }
  // ── 收集 PR 上下文 ──
  let meta;
  try {
    meta = JSON.parse(gh(`pr view ${PR} --json title,body,headRefName,files`));
  } catch (e) {
    console.error("取 PR 元信息失败:", e.message);
    process.exit(1);
  }
  let diff = "";
  try {
    diff = gh(`pr diff ${PR}`);
  } catch (e) {
    console.error("取 diff 失败:", e.message);
    process.exit(1);
  }
  let diffTruncated = false;
  if (diff.length > MAX_DIFF) {
    diff = diff.slice(0, MAX_DIFF);
    diffTruncated = true;
  }
  const fileList = (meta.files || []).map((f) => `- ${f.path} (+${f.additions}/-${f.deletions})`).join("\n");

  // ── 动态分档:按改动规模选面板配比 + 轮数 + arbiter 档(显式 pin 优先)──
  const changedFiles = (meta.files || []).length;
  const changedLines = (meta.files || []).reduce((s, f) => s + (f.additions || 0) + (f.deletions || 0), 0);
  if (AUTOSCALE && !DEBATE_MODELS_PIN) {
    const tier = pickTier(changedLines, changedFiles);
    TIER_LABEL = tier.label;
    DEBATE_MODELS = buildTierPanel(tier, { flashModel: FLASH_MODEL, proModel: PRO_MODEL, liteModel: LITE_MODEL });
    if (!MAX_ROUNDS_PIN) MAX_ROUNDS = tier.rounds;
    if (!ARBITER_PIN) ARBITER_MODEL = tier.pro > 0 ? PRO_MODEL : FLASH_MODEL;
  } else {
    TIER_LABEL = DEBATE_MODELS_PIN ? "manual-pin" : "autoscale-off";
  }
  if (!PANEL_PIN) PANEL = DEBATE_MODELS.length;

  const mix = DEBATE_MODELS.reduce((a, m) => ((a[m] = (a[m] || 0) + 1), a), {});
  console.error(
    `Pi review: PR #${PR} · 档[${TIER_LABEL}] ${changedLines} 行/${changedFiles} 文件 · ` +
      `面板 ${PANEL}×≤${MAX_ROUNDS}轮 · 收敛≥${CONFIDENCE_TARGET} · fan-out ${FANOUT_ENABLED ? `≤${FANOUT_MAX}` : "关"}\n` +
      `  gather ${GATHER_MODEL} · arbiter ${ARBITER_MODEL} · 在线 provider: ${liveProviders.join(", ")}\n` +
      `  debate 配比: ${Object.entries(mix).map(([m, n]) => `${m}×${n}`).join(", ")}`,
  );

  // plan 探测
  const pm = `${meta.title} ${meta.headRefName}`.match(/plan-[a-z0-9-]+-v\d+/i);
  let plan = null;
  if (pm) {
    plan = { name: pm[0], path: null, text: null };
    for (const dir of ["docs", "docs/finished_plans", "docs/plans-skeleton"]) {
      const p = `${dir}/${pm[0]}.md`;
      if (existsSync(p)) {
        plan.path = p;
        plan.text = readFileSync(p, "utf8").slice(0, 40000);
        break;
      }
    }
  }
  const planBlock = plan?.text
    ? `\n## 关联 Plan(${plan.path})\n${plan.text}\n`
    : plan
      ? `\n## 关联 Plan: ${plan.name}(未在仓库找到文件,按非 plan PR 处理)\n`
      : "";

  const prContext = `
## PR #${PR}: ${meta.title}
${(meta.body || "").slice(0, 4000)}

## 变更文件
${fileList || "(无)"}
${planBlock}
## 完整 diff${diffTruncated ? `(已截断至 ${MAX_DIFF} 字符)` : ""}
\`\`\`diff
${diff}
\`\`\`
`.trim();

  // ── 第零步:gather(flash 压成 brief)──
  console.error(`▶ 第零步 gather(${GATHER_MODEL})`);
  const gatherRaw = await chat(gatherPrompt(prContext), { label: `gather@${GATHER_MODEL}`, model: GATHER_MODEL });
  const gather = extractJSON(gatherRaw);
  let brief;
  if (gather) {
    const ds = (gather.diff_summary || [])
      .map((d) => `- **${d.file}**:${d.change}${d.anchors?.length ? `〔锚点 ${d.anchors.join(", ")}〕` : ""}`)
      .join("\n");
    const pc = (gather.plan_checklist || []).map((c) => `- [ ] ${c.item}${c.where ? `(→ ${c.where})` : ""}`).join("\n");
    const wt = (gather.worldview_topics || []).join("、");
    brief =
      `### Diff 摘要(逐文件)\n${ds || "(gather 未给)"}\n\n` +
      `### Plan 交付物 checklist\n${pc || (plan ? "(gather 未抽到)" : "非 plan PR")}\n\n` +
      `### 适用世界观主题\n${wt || "本次改动不涉及特定世界观主题(仍按准则通查)"}\n\n` +
      `> 完整 diff 见下(如需逐行核对)。\n${prContext}`;
  } else {
    console.error("  gather 失败/解析空,降级:直接用原始 prContext 当 brief");
    brief = prContext;
  }

  // ── 第一步:debate 轮控 ──
  const pool = []; // 全部 findings(带来源标记)
  const trajectory = []; // 每轮自信度中位数
  let lastUncertain = [];
  let extraContext = "";
  const fetched = new Set();
  let convergedRound = null;
  let finalAgg = { median: null, min: null, max: null, n: 0 };

  for (let round = 1; round <= MAX_ROUNDS; round++) {
    const cd = computeCountdown(round, MAX_ROUNDS);
    console.error(`▶ debate ${cd.label}`);
    const members = Array.from({ length: PANEL }, (_, i) => DEBATE_MODELS[i % DEBATE_MODELS.length]);
    const results = await mapLimit(members, CONCURRENCY, (model, i) =>
      chat(debatePrompt(brief, extraContext, cd, lastUncertain), {
        label: `debate.r${round}#${i}@${model}`,
        model,
      }).then(parseMember),
    );

    results.forEach((r, i) =>
      r.findings.forEach((f) => pool.push({ ...f, by: `r${round}#${i}`, round })),
    );
    const agg = aggregateConfidence(results.map((r) => r.confidence));
    finalAgg = agg;
    trajectory.push(agg.median);
    lastUncertain = dedupeUncertain(results.flatMap((r) => r.uncertain));
    const filesNeeded = [...new Set(results.flatMap((r) => r.filesNeeded))];
    console.error(
      `  自信度中位数 ${agg.median ?? "—"}(${agg.min ?? "—"}~${agg.max ?? "—"}, n=${agg.n}) · 累计 findings ${pool.length} · 弱项 ${lastUncertain.length}`,
    );

    if (agg.median !== null && agg.median >= CONFIDENCE_TARGET) {
      convergedRound = round;
      console.error(`  ✅ 达收敛阈值,提前收口于第 ${round} 轮`);
      break;
    }
    if (round === MAX_ROUNDS) {
      console.error(`  ⛔ 命中硬上限 ${MAX_ROUNDS} 轮,强制收口`);
      break;
    }
    if (MAX_FILES > 0 && filesNeeded.length) {
      const block = fetchFiles(filesNeeded, fetched);
      if (block) {
        extraContext += block;
        console.error(`  为下一轮补充了文件上下文(累计抓取 ${fetched.size} 个)`);
      }
    }
  }

  // ── 低自信精准 fan-out ──
  let fanoutDims = [];
  if (
    FANOUT_ENABLED &&
    lastUncertain.length &&
    (finalAgg.median === null || finalAgg.median < CONFIDENCE_TARGET)
  ) {
    fanoutDims = lastUncertain.slice(0, FANOUT_MAX);
    console.error(`▶ 低自信 fan-out:${fanoutDims.length} 个专项成员并行(${fanoutDims.map((d) => d.dimension).join(", ")})`);
    const fr = await mapLimit(fanoutDims, CONCURRENCY, (dim, i) =>
      chat(fanoutPrompt(brief, extraContext, dim), { label: `fanout#${i}:${dim.dimension}`, model: DEBATE_MODELS[i % DEBATE_MODELS.length] }).then(
        parseMember,
      ),
    );
    fr.forEach((r, i) => r.findings.forEach((f) => pool.push({ ...f, by: `fanout:${fanoutDims[i].dimension}`, round: "fanout" })));
  }

  const deduped = dedupeFindings(pool);

  // ── 收敛状态文案 ──
  const trajStr = trajectory.map((t) => (t === null ? "—" : `${t}`)).join(" → ");
  const finalConf = finalAgg.median;
  const converged = convergedRound !== null;
  const statusLine = converged
    ? `✅ 第 ${convergedRound} 轮收敛(自信度中位数 ${finalConf} ≥ 阈值 ${CONFIDENCE_TARGET})。轨迹:${trajStr}。`
    : `⚠️ 跑满 ${MAX_ROUNDS} 轮仍未达收敛阈值(最终自信度中位数 ${finalConf ?? "—"} < ${CONFIDENCE_TARGET})` +
      `${fanoutDims.length ? `,已对 ${fanoutDims.length} 个弱项做专项 fan-out` : ""}。轨迹:${trajStr}。**结论可靠性偏低,请人工重点复核。**`;

  // ── 第二步:arbiter 综合 ──
  console.error("▶ arbiter 综合产出 review");
  let review = await chat(arbiterPrompt(brief, deduped, statusLine, plan?.name), {
    label: `arbiter@${ARBITER_MODEL}`,
    model: ARBITER_MODEL,
  });
  if (!review) {
    review =
      `_arbiter 阶段无输出,以下为存活 findings 直出:_\n\n` +
      (deduped.length
        ? deduped.map((f) => `- [${f.severity}] ${f.file}:${f.line} — ${f.title}${f.why ? `(${f.why})` : ""}`).join("\n")
        : "未发现高置信度问题。");
  }

  // ── 组装并发布 ──
  const num = (n) => n.toLocaleString("en-US");
  const grand = Object.values(usageByModel).reduce(
    (a, m) => ({ prompt: a.prompt + m.prompt, completion: a.completion + m.completion, total: a.total + m.total, calls: a.calls + m.calls }),
    { prompt: 0, completion: 0, total: 0, calls: 0 },
  );
  const perModel = Object.entries(usageByModel)
    .sort((a, b) => b[1].total - a[1].total)
    .map(([m, u]) => `\`${m}\` ${num(u.total)}(${u.calls} 次)`)
    .join(" · ");

  const findingsTable = deduped.length
    ? `\n\n<details><summary>🔎 findings 明细(${deduped.length} 条,已去重)</summary>\n\n` +
      `| 严重度 | file:line | 问题 | consensus |\n|---|---|---|---|\n` +
      deduped
        .map((f) => `| ${f.severity} | ${f.file}:${f.line} | ${(f.title || "").replace(/\|/g, "/")} | ${f.consensus} |`)
        .join("\n") +
      `\n</details>`
    : "";

  const header =
    `## 🤖 Pi Plan-Aware Review · PR #${PR}\n\n` +
    `> 引擎:确定性轮控脚本(无界 agent 循环已退役)。**动态分档 [${TIER_LABEL}]**(${changedLines} 行/${changedFiles} 文件)→ 面板 ${PANEL} 成员 × 最多 ${MAX_ROUNDS} 轮,自信度门控 + 低自信精准 fan-out。\n` +
    `> ${statusLine}\n` +
    (plan ? `> Plan: \`${plan.name}\`${plan.path ? "" : "(未找到文件)"}\n` : "") +
    (diffTruncated ? `> ⚠️ diff 过大已截断至 ${MAX_DIFF} 字符\n` : "") +
    `> 模型:gather \`${GATHER_MODEL}\` · arbiter \`${ARBITER_MODEL}\`\n` +
    `> debate 面板配比(${PANEL} 成员):${Object.entries(mix).map(([m, n]) => `\`${m}\`×${n}`).join(" · ")}\n\n` +
    `> [!WARNING]\n` +
    `> 本审核由多模型自动 debate 生成,**不可 100% 信赖**。请自行核对 file:line 与上下文再决定是否采纳。\n\n`;

  const tokenFooter =
    `\n\n---\n📊 **本次 token 消耗**:总 **${num(grand.total)}**` +
    `(prompt ${num(grand.prompt)} + completion ${num(grand.completion)})· ${grand.calls} 次模型调用` +
    (perModel ? `\n> ${perModel}` : "") +
    `\n`;

  const body = header + review + findingsTable + tokenFooter;
  writeFileSync("/tmp/pi-review.md", body);
  if (process.env.PI_DRY_RUN) {
    console.error("— PI_DRY_RUN:不发评论,正文如下 —");
    console.log(body);
  } else {
    try {
      gh(`pr comment ${PR} --body-file /tmp/pi-review.md`);
      console.error("✅ 已发布 review 评论");
    } catch (e) {
      console.error("发布评论失败:", e.message);
      console.error(body);
      process.exit(1);
    }
  }
}

// 弱项去重(按 dimension 文本归一),合并 reason/need。
export function dedupeUncertain(items) {
  const byDim = new Map();
  for (const u of items || []) {
    if (!u || !u.dimension) continue;
    const key = String(u.dimension).trim().toLowerCase();
    const existing = byDim.get(key);
    if (!existing) {
      byDim.set(key, { dimension: String(u.dimension).trim(), reason: u.reason || "", need: u.need || "" });
    } else {
      if (u.reason && !existing.reason.includes(u.reason)) existing.reason += (existing.reason ? "; " : "") + u.reason;
      if (u.need && !existing.need.includes(u.need)) existing.need += (existing.need ? "; " : "") + u.need;
    }
  }
  return [...byDim.values()];
}

const isEntry = import.meta.url === `file://${process.argv[1]}`;
if (isEntry) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
