#!/usr/bin/env node
// 统一 Review —— PR 评论 `/review` 单触发(合并旧 `/review pi` + `/review hive`)。
//
// 设计:沿用「pi 对峙轮控 + hive 怀疑审判 + token 统计」的确定性外层编排,但把每个节点
// 从【单轮无工具 HTTP】升级为【带 read/grep 工具、能翻真实代码的 coding-agent 会话】——
// 即 hybrid:外层是脚本写死的离散轮控(天生有界),内层是有自主性的 agent。
//
// 模型路由(用户定调:claude=小模型 swarm,GPT=裁判):
//   - finder / voter swarm  → `claude -p`  跑 proxy 上的廉价小模型(deepseek/sensenova)。
//   - arbiter 总裁决        → `codex exec` 固定跑 GPT(gpt-5.5),只一个、要最强推理。
//   两个 harness 都经自家代理 proxy.kizun4.uk:claude 走 /v1/messages(anthropic 格式,
//   proxy 把它转给 deepseek/sensenova),codex 走 /v1(responses/openai 格式)跑 gpt-5.5。
//   鉴权同一把 key(REVIEW_PROXY_KEY):claude 用作 ANTHROPIC_AUTH_TOKEN,codex 用作 OPENAI_API_KEY。
//
// 流程:gather(压 brief)→ debate 轮控(claude finder 并行,自信度门控 + 低自信 fan-out)
//   → 去重 → 审判(claude 怀疑投票多轮,默认 NOT_REAL,接受/拒绝/争议分流)
//   → arbiter(codex gpt-5.5 总裁决)→ report(合并 + token 按 harness 分)→ 发评论。
//
// 自包含:只用 Node 内置模块 + `gh`/`codex`/`claude` CLI,无 npm 依赖。
// 纯逻辑(harnessOf / pickTier / dedupeFindings / aggregateConfidence / computeCountdown /
//   extractJSON / normalizeConfidence / tallyDecision …)导出供 review.test.mjs 用 `node --test` 锁行为。

import { execSync, spawn } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// ── 端点 / 鉴权(全 env 可覆盖)────────────────────────────────────────────────
const PROXY_BASE = (process.env.REVIEW_PROXY_BASE || "https://proxy.kizun4.uk").replace(/\/+$/, "");
// 一把 key 两用:claude→ANTHROPIC_AUTH_TOKEN,codex→OPENAI_API_KEY。兼容旧 secret 名。
const PROXY_KEY = process.env.REVIEW_PROXY_KEY || process.env.PI_CLIPROXY_KEY || process.env.OPENAI_API_KEY || "";
// codex 配置目录:绝对不能放 /tmp(codex 拒在临时目录建 helper binary)。CI 用 ~/.codex。
const CODEX_HOME = process.env.REVIEW_CODEX_HOME || join(process.env.HOME || "/root", ".codex");
const CODEX_SANDBOX = process.env.REVIEW_CODEX_SANDBOX || "read-only"; // review 只读,read-only 够且安全
// claude 侧跑的是小模型(deepseek/sensenova) finder/voter。**实测:小模型给工具就陷入工具循环、
// 到 max-turns 都不出最终 JSON(subtype=error_max_turns,无 result)**,既慢又不可靠。所以默认【不给工具】——
// finder/voter 纯基于 prompt 里的 diff brief + 脚本主动 fetchFiles 喂进来的周边代码出 findings
// (等价于已验证几百次的 pi/hive 单轮模式,只是经 claude harness 跑、统计更准)。
// 真正的"agent 自主翻代码"留给裁判 codex gpt-5.5(强模型才用得好工具)。
// 想给小模型开工具:设 REVIEW_CLAUDE_TOOLS="Read,Grep,Glob,..."(届时 --max-turns 生效防失控)。
const CLAUDE_TOOLS = process.env.REVIEW_CLAUDE_TOOLS ?? "";
const CLAUDE_MAX_TURNS = Math.max(1, parseInt(process.env.REVIEW_CLAUDE_MAX_TURNS || "8", 10));
// 无工具模式下注入的强约束前缀:小模型(deepseek)极易输出"我要读文件"的 tool_call 废话而非 JSON,
// 必须明确告知它没有工具 + 第一字符必须是 {(实测加此前缀 + --tools "" 后稳定出干净 findings)。
const NO_TOOL_PREFIX =
  "（运行约束:你没有任何工具,无法读取文件,只能严格基于下方提供的材料判断。" +
  "需要输出 JSON 时第一个字符必须是 `{` 或 `[`,直接给最终 JSON——禁止任何前言、解释、思考过程、tool_call 文本。）\n\n";

// ── 角色模型(env 可覆盖)──────────────────────────────────────────────────────
const FLASH_MODEL = process.env.REVIEW_FLASH_MODEL || "deepseek-v4-flash"; // claude harness 主力小模型
const LITE_MODEL = process.env.REVIEW_LITE_MODEL || "sensenova-6.7-flash-lite"; // claude harness 异源多样性
const GATHER_MODEL = process.env.REVIEW_GATHER_MODEL || FLASH_MODEL; // gather 永远用廉价 flash
const ARBITER_MODEL = process.env.REVIEW_ARBITER_MODEL || "gpt-5.5"; // 裁判固定 codex GPT
const VOTER_MODELS = expandModelList(process.env.REVIEW_VOTER_MODELS || `${FLASH_MODEL},${LITE_MODEL}`);

// ── 动态分档(autoscale 默认开)──────────────────────────────────────────────
const AUTOSCALE = process.env.REVIEW_AUTOSCALE !== "0";
const DEBATE_MODELS_PIN = process.env.REVIEW_DEBATE_MODELS ? expandModelList(process.env.REVIEW_DEBATE_MODELS) : null;
const MAX_ROUNDS_PIN = process.env.REVIEW_MAX_ROUNDS ? Math.max(1, parseInt(process.env.REVIEW_MAX_ROUNDS, 10)) : null;
const PANEL_PIN = process.env.REVIEW_PANEL ? Math.max(1, parseInt(process.env.REVIEW_PANEL, 10)) : null;
// 以下按档/pin 在 main() 敲定(故 let);此处给安全默认。
let DEBATE_MODELS = DEBATE_MODELS_PIN || expandModelList(`${FLASH_MODEL}*3`);
let MAX_ROUNDS = MAX_ROUNDS_PIN || 2;
let PANEL = PANEL_PIN || DEBATE_MODELS.length;
let TIER_LABEL = "default";

// ── 轮控 / 投票 / 并发参数 ───────────────────────────────────────────────────
const CONFIDENCE_TARGET = clampPct(parseFloat(process.env.REVIEW_CONFIDENCE_TARGET || "80"), 80);
const CONCURRENCY = Math.max(1, parseInt(process.env.REVIEW_CONCURRENCY || "4", 10)); // agent 进程重,默认收紧
const FANOUT_ENABLED = process.env.REVIEW_FANOUT !== "0";
const FANOUT_MAX = Math.max(1, parseInt(process.env.REVIEW_FANOUT_MAX || "4", 10));
const VOTERS = Math.max(1, parseInt(process.env.REVIEW_VOTERS || "5", 10));
const MAX_VOTE_ROUNDS = Math.max(1, parseInt(process.env.REVIEW_VOTE_ROUNDS || "2", 10));
const ACCEPT_RATIO = clampRatio(process.env.REVIEW_ACCEPT_RATIO, 0.7); // REAL 占比 ≥ 此 → 接受
const REJECT_RATIO = clampRatio(process.env.REVIEW_REJECT_RATIO, 0.35); // REAL 占比 ≤ 此 → 拒绝
const TOP_N = Math.max(1, parseInt(process.env.REVIEW_TOP_N || "8", 10)); // 交裁判的存活上限
// 单次 agent 会话硬超时:agent 多轮 + 工具远慢于单轮 HTTP。codex 裁判额外放宽。
const CALL_TIMEOUT_MS = Math.max(60_000, parseInt(process.env.REVIEW_CALL_TIMEOUT_MS || "300000", 10));
const ARBITER_TIMEOUT_MS = Math.max(CALL_TIMEOUT_MS, parseInt(process.env.REVIEW_ARBITER_TIMEOUT_MS || "480000", 10));
const RETRIES = Math.max(1, parseInt(process.env.REVIEW_RETRIES || "2", 10));
const ARBITER_RETRIES = Math.max(RETRIES, parseInt(process.env.REVIEW_ARBITER_RETRIES || "3", 10)); // gpt-5.5 易过载,多试

const MAX_FILES = Math.max(0, parseInt(process.env.REVIEW_MAX_FILES || "6", 10));
const FILE_BYTES = Math.max(1000, parseInt(process.env.REVIEW_FILE_BYTES || "8000", 10));
const MAX_DIFF = parseInt(process.env.REVIEW_MAX_DIFF || "200000", 10);
const DRY_RUN = !!process.env.REVIEW_DRY_RUN;

const PR = process.env.PR_NUMBER;

// token 记账:model → {prompt, completion, total, calls, harness}
const usageByModel = {};
function recordUsage(model, harness, prompt, completion) {
  const m =
    usageByModel[model] || (usageByModel[model] = { prompt: 0, completion: 0, total: 0, calls: 0, harness });
  m.prompt += prompt || 0;
  m.completion += completion || 0;
  m.total += (prompt || 0) + (completion || 0);
  m.calls += 1;
}

// ══════════════════════════════════════════════════════════════════════════════
//  纯逻辑(导出供测试)
// ══════════════════════════════════════════════════════════════════════════════

// 模型 → harness 路由:GPT 系(gpt/o1/o3/codex/openai/chatgpt 开头)走 codex,其余走 claude。
export function harnessOf(model) {
  return /^(gpt|o\d|codex|openai|chatgpt)/i.test(String(model || "").trim()) ? "codex" : "claude";
}

// [0,100] 整数百分比;非法返回 fallback(默认 null = 无效,不计入聚合)。
export function normalizeConfidence(v, fallback = null) {
  const n = typeof v === "string" ? parseFloat(v) : v;
  if (typeof n !== "number" || !Number.isFinite(n)) return fallback;
  return Math.max(0, Math.min(100, Math.round(n)));
}

function clampPct(v, fallback) {
  const n = normalizeConfidence(v);
  return n === null ? fallback : n;
}

function clampRatio(v, fallback) {
  const n = typeof v === "string" ? parseFloat(v) : v;
  if (typeof n !== "number" || !Number.isFinite(n) || n < 0 || n > 1) return fallback;
  return n;
}

// 面板自信度聚合:中位数做收敛门控(抗离群),附 min/max/n。
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
        `【第 1/${maxRounds} 轮 · 从容核查】本轮全面排查,不急于下结论。你有 Read/Grep 工具,` +
        `请打开仓库真实文件核对每条 finding 的 file:line,别只凭 diff 推测。把还没把握的写进 ` +
        `uncertain_dimensions,把还想看的代码写进 files_needed。`,
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

// 跨轮/跨成员去重:同 file|line|标题(归一化)合并,取最高严重度,累计 consensus(几人独立提出)。
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
        key,
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

// 怀疑投票分流:给一条 finding 的累计票数(real/not),按接受/拒绝阈值 + 是否末轮裁断。
// 返回 'accept' | 'reject' | 'pending'(争议,进下一轮)。末轮强制收口:未达接受阈值一律 reject(从严)。
export function tallyDecision(real, not, { acceptRatio = ACCEPT_RATIO, rejectRatio = REJECT_RATIO, isFinalRound = false } = {}) {
  const total = real + not;
  if (total === 0) return isFinalRound ? "reject" : "pending"; // 没票:末轮从严拒,否则下轮再投
  const ratio = real / total;
  if (ratio >= acceptRatio) return "accept";
  if (ratio <= rejectRatio) return "reject";
  return isFinalRound ? "reject" : "pending";
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

// 展开 `name*count` 语法:`"a*3, b, c*2"` → ["a","a","a","b","c","c"]。力大砖飞的人体工学入口。
export function expandModelList(spec) {
  return String(spec || "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .flatMap((tok) => {
      const m = tok.match(/^(.+?)\*(\d+)$/);
      if (m) return Array.from({ length: Math.max(0, parseInt(m[2], 10)) }, () => m[1].trim());
      return [tok];
    });
}

// 动态分档表(均衡档):按总变更行数选档,定 flash/lite finder 数 + 轮数。小改省钱、大改力大砖飞。
// finder/voter 全走 claude 小模型;arbiter 恒 codex gpt-5.5,不在此表。maxLines 升序,末档 Infinity 兜底。
export const TIERS = [
  { label: "trivial", maxLines: 40, flash: 2, lite: 0, rounds: 1 },
  { label: "small", maxLines: 250, flash: 3, lite: 1, rounds: 1 },
  { label: "medium", maxLines: 900, flash: 4, lite: 2, rounds: 2 },
  { label: "large", maxLines: 2500, flash: 6, lite: 3, rounds: 2 },
  { label: "huge", maxLines: Infinity, flash: 8, lite: 4, rounds: 2 },
];

// 选档:总变更行数定基础档;文件数 ≥ bumpFiles(改动面广)升一档,封顶末档。
export function pickTier(changedLines, changedFiles, { tiers = TIERS, bumpFiles = 15 } = {}) {
  const lines = Number(changedLines) || 0;
  let idx = tiers.findIndex((t) => lines <= t.maxLines);
  if (idx < 0) idx = tiers.length - 1;
  if ((Number(changedFiles) || 0) >= bumpFiles && idx < tiers.length - 1) idx += 1;
  return tiers[idx];
}

// 把一档的 flash/lite 数量按角色模型名拼成展开后的面板清单(数量为 0 的角色略过)。
export function buildTierPanel(tier, { flashModel = FLASH_MODEL, liteModel = LITE_MODEL } = {}) {
  const parts = [];
  if (tier.flash > 0) parts.push(`${flashModel}*${tier.flash}`);
  if (tier.lite > 0) parts.push(`${liteModel}*${tier.lite}`);
  return expandModelList(parts.join(","));
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

// ══════════════════════════════════════════════════════════════════════════════
//  审核准则(静态 worldview slice,塞进每个 agent 上下文)
// ══════════════════════════════════════════════════════════════════════════════
const GUIDELINES = `
## Bong 项目审核准则(末法残土修仙沙盒,三层架构 server/Rust + agent/TS + client/Java)

### 核对要求
严格基于提供的 diff 与「补充代码上下文」判断 file:line + 周围代码是否已处理;拿不准、无法定位的写进 uncertain_dimensions,别凭空推测。

### 世界观锚定(docs/worldview.md 是正典,代码不得矛盾)
- 六境界:醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚。禁用"筑基/金丹/元婴"等传统称谓。
- 货币:骨币(通货)、灵石(燃料,非货币)。不得出现"灵石=钱"的逻辑。
- 灵气守恒律:全服灵气总量恒定。真元流动必须走 qi_physics::QiTransfer{from,to,amount}。红旗:
  qi_current += X 无对应 zone 减 / zone.spirit_qi -= Y 无对应玩家增 / 衰变让真元凭空消失 / 招式只扣攻方不写环境。
  **注意**:若某模式与已合并的 sibling(如 fauna/rat_phase.rs)逐字一致,属既有正典模式,非本 PR 引入,勿当 blocker。
- 物理常数唯一源:衰减/逸散/半衰常数必须来自 qi_physics,禁止各 plan 硬编 *_DECAY*/*_DRAIN*/0.0X_f64。
- 命名"末法去上古",不要仙气飘飘。

### 架构硬约束
- 跨层通信只走 Redis IPC + CustomPayload。
- IPC schema:TypeBox(TS)是 server↔agent 的 source of truth → JSON Schema → Rust serde,改 schema 必须双端同步。
  **注意**:server→client(Rust→Java)的 CustomPayload 是另一条通道,全仓既有几十个 *_emit.rs 都手写 serde JSON,
  不强制走 TypeBox——别把 agent-IPC 的约束误套到 client payload。
- 新增 SkillRegistry::register 必须同步在 cultivation::meridian::severed::SkillMeridianDependencies::declare 注册依赖经脉。
- Bevy ECS:component 是数据、system 是逻辑,别在 component 写逻辑方法。

### 代码与测试
- 简单易懂 > 花里胡哨;过度抽象/无用注释/超出 plan 的功能蔓延都是减分项。
- 测契约不测实现;测试要饱和:happy path + 所有边界 + 所有错误分支 + 所有状态转换。
`.trim();

// ══════════════════════════════════════════════════════════════════════════════
//  IO 工具 + harness 执行层
// ══════════════════════════════════════════════════════════════════════════════
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

// spawn 一个 CLI,prompt 经 stdin 传(避免 argv 超限 + shell 注入;shell:false 默认),收集 stdout/stderr,硬超时 kill。
function runCli(cmd, args, { env = {}, input = "", timeoutMs = CALL_TIMEOUT_MS } = {}) {
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(cmd, args, { env: { ...process.env, ...env }, stdio: ["pipe", "pipe", "pipe"] });
    } catch (e) {
      resolve({ code: -1, stdout: "", stderr: `spawn 失败: ${e.message}` });
      return;
    }
    let out = "";
    let err = "";
    let done = false;
    const finish = (r) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      resolve(r);
    };
    const timer = setTimeout(() => {
      try {
        child.kill("SIGKILL");
      } catch {
        /* ignore */
      }
      finish({ code: -2, stdout: out, stderr: err + "\n[超时 kill]" });
    }, timeoutMs);
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (err += d));
    child.on("error", (e) => finish({ code: -1, stdout: out, stderr: err + String(e) }));
    child.on("close", (code) => finish({ code, stdout: out, stderr: err }));
    try {
      if (input) child.stdin.write(input);
      child.stdin.end();
    } catch {
      /* stdin 可能已关 */
    }
  });
}

// 解析 claude -p --output-format json 的输出:取 .result 文本 + .usage token。
function parseClaudeOut(stdout) {
  const j = extractJSON(stdout);
  if (!j) return { text: null, prompt: 0, completion: 0 };
  const text = typeof j.result === "string" ? j.result : null;
  const u = j.usage || {};
  const prompt = (u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0);
  const completion = u.output_tokens || 0;
  return { text: text && text.trim() ? text : null, prompt, completion };
}

// 解析 codex exec --json(JSONL 事件流)的 token;最终文本由 -o <file> 旁路读取。
// codex 事件格式跨版本不稳,这里宽容扫描:累计任意含 input/output token 的事件,取最后一次(累计量)。
function parseCodexTokens(stdout) {
  let prompt = 0;
  let completion = 0;
  for (const line of String(stdout || "").split("\n")) {
    const s = line.trim();
    if (!s || s[0] !== "{") continue;
    let ev;
    try {
      ev = JSON.parse(s);
    } catch {
      continue;
    }
    const info = ev?.info || ev?.usage || ev?.token_usage || ev?.total_token_usage || ev?.msg?.info || ev;
    const tu = info?.total_token_usage || info?.token_usage || info?.usage || info;
    const inTok = tu?.input_tokens ?? tu?.prompt_tokens;
    const outTok = tu?.output_tokens ?? tu?.completion_tokens;
    if (typeof inTok === "number") prompt = inTok;
    if (typeof outTok === "number") completion = outTok;
  }
  return { prompt, completion };
}

// 统一 agent 调用:按 model 路由到 codex/claude harness,prompt 经 stdin,带 retry + 超时 + token 记账 + graceful degrade。
// 失败返回 null(上层跳过不阻断,沿用既有设计)。
async function runAgent(prompt, { label = "", model = FLASH_MODEL, retries = RETRIES, timeoutMs = CALL_TIMEOUT_MS } = {}) {
  if (!PROXY_KEY) {
    console.error(`[${label}] 无 REVIEW_PROXY_KEY,跳过`);
    return null;
  }
  const harness = harnessOf(model);
  for (let attempt = 1; attempt <= retries; attempt++) {
    let res;
    let text = null;
    let prompt_tok = 0;
    let completion_tok = 0;
    if (harness === "codex") {
      const lastMsgFile = join(mkdtempSync(join(tmpdir(), "codex-out-")), "last.txt");
      const args = [
        "exec",
        "--json",
        "--skip-git-repo-check",
        "-s",
        CODEX_SANDBOX,
        "-m",
        model,
        "-o",
        lastMsgFile,
        "--color",
        "never",
      ];
      res = await runCli("codex", args, {
        env: { CODEX_HOME, OPENAI_API_KEY: PROXY_KEY },
        input: prompt,
        timeoutMs,
      });
      try {
        if (existsSync(lastMsgFile)) text = readFileSync(lastMsgFile, "utf8");
      } catch {
        /* ignore */
      }
      const tok = parseCodexTokens(res.stdout);
      prompt_tok = tok.prompt;
      completion_tok = tok.completion;
      try {
        rmSync(lastMsgFile.replace(/\/last\.txt$/, ""), { recursive: true, force: true });
      } catch {
        /* ignore */
      }
    } else {
      // 默认【禁所有工具(--tools "") + NO_TOOL_PREFIX 强约束】:实测小模型给工具就陷工具循环、
      // 或输出 tool_call 废话文本而非 JSON;禁工具 + 前缀后 ~20s 稳定出干净 findings。
      // 仅当显式配了 CLAUDE_TOOLS 才开工具 + 多轮(CLAUDE_MAX_TURNS 防失控)。
      const args = ["-p", "--model", model, "--output-format", "json", "--bare"];
      let cInput = prompt;
      if (CLAUDE_TOOLS) {
        args.push("--allowedTools", CLAUDE_TOOLS, "--max-turns", String(CLAUDE_MAX_TURNS));
      } else {
        args.push("--tools", "");
        cInput = NO_TOOL_PREFIX + prompt;
      }
      res = await runCli("claude", args, {
        env: { ANTHROPIC_BASE_URL: PROXY_BASE, ANTHROPIC_AUTH_TOKEN: PROXY_KEY },
        input: cInput,
        timeoutMs,
      });
      const parsed = parseClaudeOut(res.stdout);
      text = parsed.text;
      prompt_tok = parsed.prompt;
      completion_tok = parsed.completion;
    }
    if (text && text.trim()) {
      recordUsage(model, harness, prompt_tok, completion_tok);
      return text;
    }
    console.error(
      `[${label}] ${model}(${harness}) 第 ${attempt}/${retries} 次无输出 (code=${res.code}) ${String(res.stderr || "").slice(-200)}`,
    );
    if (attempt < retries) await sleep(3000 * attempt);
  }
  return null;
}

// 抓取成员请求的补充文件(本地签出版本)。去重 + 上限 + 截断 + 路径安全过滤。
function fetchFiles(paths, already) {
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
  return `\n## 补充代码上下文\n${picked.join("\n\n")}`;
}

// ══════════════════════════════════════════════════════════════════════════════
//  提示词
// ══════════════════════════════════════════════════════════════════════════════
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
  `你是 Bong 项目 PR 审核的一员(深度推理,独立判断,带 Read/Grep 工具)。\n${GUIDELINES}\n\n` +
  `## 已整理的审核 brief\n${brief}\n` +
  (priorUncertain.length
    ? `\n## 上一轮面板标记的弱项(请优先攻克)\n${priorUncertain.map((u) => `- ${u.dimension}: ${u.reason || ""} ${u.need ? `(需:${u.need})` : ""}`).join("\n")}\n`
    : "") +
  `${extraContext}\n\n` +
  `${countdown.text}\n\n` +
  `基于以上材料 + 你打开真实文件的核对,逐项审核本 PR:\n` +
  `a) plan 该阶段交付物是否逐项落地(对照 plan_checklist 标 ✅/❌/⚠️)\n` +
  `b) plan 列出但 PR 缺失的模块/函数/测试/schema\n` +
  `c) 正确性:逻辑、边界 off-by-one、并发、错误分支、IPC schema 对齐\n` +
  `d) 世界观对齐:六境界命名 / 骨币灵石 / 末法风格\n` +
  `e) 灵气守恒:真元增减是否走 qi_physics::QiTransfer、物理常数来源是否唯一(注意 sibling 既有模式不算本 PR 引入)\n` +
  `f) 测试饱和:happy + 边界 + 错误分支 + 状态转换\n` +
  `g) 简洁度:过度抽象 / 冗余注释 / 超出 plan 的功能蔓延\n\n` +
  `每条 finding 必须带 file:line + diff 证据片段。**没把握 / 推测性的不要当 finding 报**,而是写进 ` +
  `uncertain_dimensions 说明你还差什么、把想看的文件写进 files_needed(下一轮会补给你)。没问题就空数组,严禁凑数。\n\n` +
  `只输出纯 JSON(不要解释文字):\n` +
  `{\n` +
  `  "confidence": 0-100,\n` +
  `  "findings": [{"severity":"blocker|major|minor","file":"路径","line":"行号/范围","title":"一句话问题","evidence":"代码","why":"为什么是问题"}],\n` +
  `  "uncertain_dimensions": [{"dimension":"a-g 之一或具体主题","reason":"为何还没把握","need":"还需要看什么才能定论"}],\n` +
  `  "files_needed": ["还想查看的仓库文件路径(相对仓库根)"]\n` +
  `}`;

const fanoutPrompt = (brief, extraContext, dim) =>
  `你是 Bong PR 审核的【${dim.dimension}】专项核查员(带 Read/Grep 工具)。前几轮对该维度把握不足,现在只盯这一维度深挖到底。\n` +
  `${GUIDELINES}\n\n## 审核 brief\n${brief}\n${extraContext}\n\n` +
  `## 待澄清\n${dim.reason || ""}${dim.need ? `(还需:${dim.need})` : ""}\n\n` +
  `打开真实文件核对,只就【${dim.dimension}】给结论:有问题逐条带 file:line + 证据;确认无问题就明说"该维度核查通过"。\n` +
  `只输出纯 JSON:\n` +
  `{"dimension":"${dim.dimension}","confidence":0-100,"verdict":"问题|通过|仍不确定",` +
  `"findings":[{"severity":"blocker|major|minor","file":"路径","line":"行号","title":"...","evidence":"...","why":"..."}]}`;

const votePrompt = (prContext, activeFindings, askMissed) => {
  const view = activeFindings.map(({ id, severity, file, line, title, why }) => ({ id, severity, file, line, title, why }));
  return (
    `你是 Bong PR 审核的**怀疑型投票者**(带 Read/Grep 工具)。\n${GUIDELINES}\n\n${prContext}\n\n` +
    `逐条裁断下面的 findings(带 id)。**默认判 NOT_REAL**——只有当你打开真实文件、明确确认全部四点:\n` +
    `① 问题真实存在 ② 该代码路径可达 ③ 周围代码确实没有处理 ④ 行号对得上,才投 REAL。\n` +
    `证据不足、无法定位、推测性、风格洁癖、"可能/也许/建议"一律 NOT_REAL。宁可漏过,也不要放过假阳性。\n\n` +
    `findings:\n${JSON.stringify(view, null, 2)}\n\n` +
    (askMissed ? `如确有**所有人都漏掉、且证据确凿**的真问题,补进 missed(没有就空数组,严禁硬凑)。\n` : ``) +
    `只输出纯 JSON(不要解释文字):\n` +
    `{"votes":[{"id":1,"verdict":"REAL|NOT_REAL"}]` +
    (askMissed ? `,"missed":[{"severity":"blocker|major|minor","file":"路径","line":"行号","title":"...","why":"..."}]` : ``) +
    `}`
  );
};

const arbiterPrompt = (brief, survived, status, planName) =>
  `你是本次审核的**总裁判**(综合 + 发布,从严,宁缺勿滥,带 Read/Grep 工具可复核)。\n${GUIDELINES}\n\n` +
  `## 审核 brief\n${brief}\n\n` +
  `## 经对峙 + 怀疑投票存活的 findings(已去重,consensus = 几人独立提出,votes = 投票通过率)\n` +
  `${survived.map((f) => `- [${f.severity}] ${f.file}:${f.line} — ${f.title}${f.why ? `(${f.why})` : ""} 〔consensus ${f.consensus}${f.voteReal != null ? ` · 票 ${f.voteReal}/${f.voteTotal}` : ""}〕`).join("\n") || "(无)"}\n\n` +
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

// ══════════════════════════════════════════════════════════════════════════════
//  主流程
// ══════════════════════════════════════════════════════════════════════════════
async function main() {
  // PR 是唯一被插进 gh shell 命令的外部值——强制数字,杜绝命令注入。
  if (!PR || !/^\d+$/.test(String(PR))) {
    console.error("PR_NUMBER 未设置或非法(必须是纯数字)");
    process.exit(1);
  }
  if (!PROXY_KEY) {
    console.error("没有 REVIEW_PROXY_KEY(在 repo secrets 配 REVIEW_PROXY_KEY 或复用 PI_CLIPROXY_KEY)");
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

  // ── 动态分档:按改动规模选 finder 面板配比 + 轮数 ──
  const changedFiles = (meta.files || []).length;
  const changedLines = (meta.files || []).reduce((s, f) => s + (f.additions || 0) + (f.deletions || 0), 0);
  if (AUTOSCALE && !DEBATE_MODELS_PIN) {
    const tier = pickTier(changedLines, changedFiles);
    TIER_LABEL = tier.label;
    DEBATE_MODELS = buildTierPanel(tier, { flashModel: FLASH_MODEL, liteModel: LITE_MODEL });
    if (!MAX_ROUNDS_PIN) MAX_ROUNDS = tier.rounds;
  } else {
    TIER_LABEL = DEBATE_MODELS_PIN ? "manual-pin" : "autoscale-off";
  }
  if (!PANEL_PIN) PANEL = DEBATE_MODELS.length;

  const mix = DEBATE_MODELS.reduce((a, m) => ((a[m] = (a[m] || 0) + 1), a), {});
  console.error(
    `Review: PR #${PR} · 档[${TIER_LABEL}] ${changedLines} 行/${changedFiles} 文件 · ` +
      `finder ${PANEL}×≤${MAX_ROUNDS}轮 · 收敛≥${CONFIDENCE_TARGET} · 投票 ${VOTERS}×≤${MAX_VOTE_ROUNDS}轮\n` +
      `  gather ${GATHER_MODEL}(claude) · arbiter ${ARBITER_MODEL}(codex) · finder 配比 ${Object.entries(mix).map(([m, n]) => `${m}×${n}`).join(", ")}`,
  );

  // ── plan 探测 ──
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

  // ── 第零步:gather(claude flash 压成 brief)──
  console.error(`▶ 第零步 gather(${GATHER_MODEL})`);
  const gatherRaw = await runAgent(gatherPrompt(prContext), { label: `gather@${GATHER_MODEL}`, model: GATHER_MODEL });
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

  // ── 第一步:debate 轮控(claude finder 并行)──
  const pool = [];
  const trajectory = [];
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
      runAgent(debatePrompt(brief, extraContext, cd, lastUncertain), {
        label: `debate.r${round}#${i}@${model}`,
        model,
      }).then(parseMember),
    );
    results.forEach((r, i) => r.findings.forEach((f) => pool.push({ ...f, by: `r${round}#${i}`, round })));
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
  if (FANOUT_ENABLED && lastUncertain.length && (finalAgg.median === null || finalAgg.median < CONFIDENCE_TARGET)) {
    fanoutDims = lastUncertain.slice(0, FANOUT_MAX);
    console.error(`▶ 低自信 fan-out:${fanoutDims.length} 个专项成员并行(${fanoutDims.map((d) => d.dimension).join(", ")})`);
    const fr = await mapLimit(fanoutDims, CONCURRENCY, (dim, i) =>
      runAgent(fanoutPrompt(brief, extraContext, dim), {
        label: `fanout#${i}:${dim.dimension}`,
        model: DEBATE_MODELS[i % DEBATE_MODELS.length],
      }).then(parseMember),
    );
    fr.forEach((r, i) => r.findings.forEach((f) => pool.push({ ...f, by: `fanout:${fanoutDims[i].dimension}`, round: "fanout" })));
  }

  const deduped = dedupeFindings(pool).map((f, i) => ({ ...f, id: i + 1 }));
  console.error(`▶ 去重后 ${deduped.length} 条 findings,进入怀疑投票审判`);

  // ── 第二步:审判(claude 怀疑型投票多轮,默认 NOT_REAL,接受/拒绝/争议分流)──
  const tally = {};
  deduped.forEach((f) => (tally[f.id] = { real: 0, not: 0 }));
  const decision = {}; // id -> 'accept' | 'reject'
  let active = deduped.map((f) => f.id);
  const missedPool = [];

  for (let round = 1; round <= MAX_VOTE_ROUNDS && active.length > 0; round++) {
    const isFinalRound = round === MAX_VOTE_ROUNDS;
    const activeFindings = deduped.concat(missedPool).filter((f) => active.includes(f.id));
    if (!activeFindings.length) break;
    console.error(`▶ 审判第 ${round}/${MAX_VOTE_ROUNDS} 轮:${VOTERS} 投票者 × ${activeFindings.length} 条未决`);
    const askMissed = round === 1;
    const voters = Array.from({ length: VOTERS }, (_, i) => VOTER_MODELS[i % VOTER_MODELS.length]);
    const results = await mapLimit(voters, CONCURRENCY, (model, i) =>
      runAgent(votePrompt(prContext, activeFindings, askMissed), { label: `vote.r${round}#${i}@${model}`, model }).then(
        (r) => extractJSON(r) || { votes: [], missed: [] },
      ),
    );
    for (const r of results) {
      for (const v of r?.votes || []) {
        const t = tally[v?.id];
        if (!t) continue;
        if (String(v.verdict || "").toUpperCase().includes("NOT")) t.not++;
        else t.real++;
      }
      if (askMissed) {
        for (const m of r?.missed || []) {
          if (m && (m.title || m.file)) {
            const id = deduped.length + missedPool.length + 1;
            const f = { ...m, id, by: "补漏", consensus: 1, severity: String(m.severity || "minor").toLowerCase() };
            missedPool.push(f);
            tally[id] = { real: 0, not: 0 };
            active.push(id);
          }
        }
      }
    }
    // 累计占比分流
    const stillActive = [];
    for (const id of active) {
      const t = tally[id];
      const verdict = tallyDecision(t.real, t.not, { isFinalRound });
      if (verdict === "pending") stillActive.push(id);
      else decision[id] = verdict;
    }
    active = stillActive;
  }

  const allFindings = deduped.concat(missedPool);
  const survived = allFindings
    .filter((f) => decision[f.id] === "accept")
    .map((f) => ({ ...f, voteReal: tally[f.id]?.real ?? 0, voteTotal: (tally[f.id]?.real ?? 0) + (tally[f.id]?.not ?? 0) }))
    .sort((a, b) => sevRank(a.severity) - sevRank(b.severity) || b.consensus - a.consensus)
    .slice(0, TOP_N);
  console.error(`▶ 审判存活 ${survived.length}/${allFindings.length} 条(交裁判 top-${Math.min(TOP_N, survived.length)})`);

  // ── 收敛状态文案 ──
  const trajStr = trajectory.map((t) => (t === null ? "—" : `${t}`)).join(" → ");
  const finalConf = finalAgg.median;
  const converged = convergedRound !== null;
  const statusLine = converged
    ? `✅ 第 ${convergedRound} 轮收敛(自信度中位数 ${finalConf} ≥ 阈值 ${CONFIDENCE_TARGET})。轨迹:${trajStr}。`
    : `⚠️ 跑满 ${MAX_ROUNDS} 轮仍未达收敛阈值(最终自信度中位数 ${finalConf ?? "—"} < ${CONFIDENCE_TARGET})` +
      `${fanoutDims.length ? `,已对 ${fanoutDims.length} 个弱项做专项 fan-out` : ""}。轨迹:${trajStr}。**结论可靠性偏低,请人工重点复核。**`;

  // ── 第三步:arbiter(codex gpt-5.5 总裁决)──
  console.error(`▶ arbiter 综合产出 review(${ARBITER_MODEL} via codex)`);
  let review = await runAgent(arbiterPrompt(brief, survived, statusLine, plan?.name), {
    label: `arbiter@${ARBITER_MODEL}`,
    model: ARBITER_MODEL,
    retries: ARBITER_RETRIES,
    timeoutMs: ARBITER_TIMEOUT_MS,
  });
  if (!review) {
    console.error("  arbiter 无输出(codex gpt-5.5 可能过载),降级:直出存活 findings");
    review =
      `_arbiter 阶段无输出(裁判模型可能过载),以下为审判存活 findings 直出:_\n\n` +
      (survived.length
        ? survived.map((f) => `- [${f.severity}] ${f.file}:${f.line} — ${f.title}${f.why ? `(${f.why})` : ""}`).join("\n")
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
    .map(([m, u]) => `\`${m}\`(${u.harness}) ${num(u.total)}(${u.calls} 次)`)
    .join(" · ");

  const findingsTable = allFindings.length
    ? `\n\n<details><summary>🔎 findings 明细(去重 ${allFindings.length} 条 → 审判存活 ${survived.length} 条)</summary>\n\n` +
      `| 严重度 | file:line | 问题 | consensus | 裁定 |\n|---|---|---|---|---|\n` +
      allFindings
        .map((f) => {
          const t = tally[f.id] || { real: 0, not: 0 };
          const st = decision[f.id] === "accept" ? `✅ ${t.real}/${t.real + t.not}` : `✗ ${t.real}/${t.real + t.not}`;
          return `| ${f.severity} | ${f.file}:${f.line} | ${(f.title || "").replace(/\|/g, "/")} | ${f.consensus} | ${st} |`;
        })
        .join("\n") +
      `\n</details>`
    : "";

  const header =
    `## 🔭 Review · PR #${PR}\n\n` +
    `> 引擎:对峙(claude finder swarm)+ 怀疑投票审判 + 裁决(codex gpt-5.5)。**动态分档 [${TIER_LABEL}]**(${changedLines} 行/${changedFiles} 文件)→ finder ${PANEL} 成员 × 最多 ${MAX_ROUNDS} 轮,自信度门控 + 低自信 fan-out → ${VOTERS} 怀疑投票者 × 最多 ${MAX_VOTE_ROUNDS} 轮。\n` +
    `> ${statusLine}\n` +
    (plan ? `> Plan: \`${plan.name}\`${plan.path ? "" : "(未找到文件)"}\n` : "") +
    (diffTruncated ? `> ⚠️ diff 过大已截断至 ${MAX_DIFF} 字符\n` : "") +
    `> 模型:finder/voter \`${FLASH_MODEL}\`+\`${LITE_MODEL}\`(claude harness · proxy) · 裁判 \`${ARBITER_MODEL}\`(codex harness · proxy)\n` +
    `> 统计:去重 ${allFindings.length} → 审判存活 ${survived.length} → 交裁判 top-${Math.min(TOP_N, survived.length)}\n\n` +
    `> [!WARNING]\n` +
    `> 本审核由多模型 agent 自动 debate + 投票生成,**不可 100% 信赖**。请自行核对 file:line 与上下文再决定是否采纳。\n\n`;

  const tokenFooter =
    `\n\n---\n📊 **本次 token 消耗**:总 **${num(grand.total)}**` +
    `(prompt ${num(grand.prompt)} + completion ${num(grand.completion)})· ${grand.calls} 次 agent 调用` +
    (perModel ? `\n> ${perModel}` : "") +
    `\n`;

  const body = header + review + findingsTable + tokenFooter;
  writeFileSync("/tmp/review.md", body);
  if (DRY_RUN) {
    console.error("— REVIEW_DRY_RUN:不发评论,正文如下 —");
    console.log(body);
  } else {
    try {
      gh(`pr comment ${PR} --body-file /tmp/review.md`);
      console.error("✅ 已发布 review 评论");
    } catch (e) {
      console.error("发布评论失败:", e.message);
      console.error(body);
      process.exit(1);
    }
  }
}

const isEntry = import.meta.url === `file://${process.argv[1]}`;
if (isEntry) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
