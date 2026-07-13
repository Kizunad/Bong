#!/usr/bin/env node
// Review v4 (Claude Code 引擎) —— 用 claude CLI 走 shuaiapi Anthropic 协议做博弈式 PR 审核。
//
// 与 v3(codex, review.mjs) 的区别：
// 1. 引擎从 codex CLI 换成 claude CLI（ANTHROPIC_BASE_URL=shuaiapi，Anthropic /v1/messages）。
// 2. 模型分层：reviewer 全用性价比模型 gpt-5.6-terra；唯一的“决策”步用 gpt-5.6-sol + ultra thinking。
// 3. 结构：4 个维度【串行】覆盖，每个维度内【控方 / 辩方并行博弈】(initial + debate 两轮)。
// 4. 硬并发限制：所有 claude 调用统一过全局信号量，且并发上限被编译期常量 HARD_MAX_CONCURRENCY 钳死。
//
// 依赖：Node 内置模块 + gh + claude CLI。无 npm runtime dependency。

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PR = process.env.PR_NUMBER;

// ── 模型分层（映射：sonnet→terra 性价比 / opus→sol 决策 / haiku→luna 更省）────────
const MODEL_REVIEWER = process.env.REVIEW_MODEL_REVIEWER || "gpt-5.6-terra";
const MODEL_DECISION = process.env.REVIEW_MODEL_DECISION || "gpt-5.6-sol";

// ── 硬并发限制：编译期天花板，env 只能往下调，绝不能往上超 ──────────────────────
const HARD_MAX_CONCURRENCY = 3;

const MAX_DIFF = intEnv("REVIEW_MAX_DIFF", 40_000, 10_000);
const MAX_PLAN = intEnv("REVIEW_MAX_PLAN", 20_000, 5_000);
const CLAUDE_TIMEOUT_MS = intEnv("REVIEW_CLAUDE_TIMEOUT_MS", 900_000, 120_000);
const CONCURRENCY = clampConcurrency(intEnv("REVIEW_CONCURRENCY", 2, 1), HARD_MAX_CONCURRENCY);
// 决策步的 ultra 思考预算（claude 的 extended thinking → 经 shuaiapi 映射为高 reasoning effort）。
// 留足输出空间：模型 max output ≈ 32k，思考预算封顶 24k，给最终 JSON 裁决留 ~8k。
const DECISION_THINKING_TOKENS = intEnv("REVIEW_DECISION_THINKING_TOKENS", 24_000, 1_024);
const DRY_RUN = /^(1|true|yes)$/i.test(String(process.env.REVIEW_DRY_RUN || "").trim());
const FAIL_ON_GATE = process.env.REVIEW_FAIL_ON_GATE !== "0";

// 4 个维度：串行覆盖，一次只审一个维度。
const DIMENSIONS = [
  {
    id: "A",
    name: "Plan 原意核查",
    focus: "确认 PR 是否真正符合关联 plan 的原意、阶段交付物和验收边界；缺 plan 时说明不适用。",
  },
  {
    id: "B",
    name: "运行接线核查",
    focus: "专盯定义未接入、emit 无消费、registry 未加载、server/client/agent/schema 单向 stub。",
  },
  {
    id: "C",
    name: "正确性与守恒核查",
    focus: "核查逻辑边界、并发/状态机、schema 契约、真元/灵气守恒和物理常数来源。",
  },
  {
    id: "D",
    name: "代码质量与测试核查",
    focus: "核查代码质量、抽象克制、注释是否简洁易懂、测试是否覆盖 happy/boundary/error/state transition。",
  },
];

// 每个维度内的两个对立角色：并行博弈。
const ROLES = [
  {
    id: "prosecutor",
    name: "控方",
    stance:
      "你要在这个维度下尽力找出所有真实、可核验、可达的问题。默认怀疑，宁可多报也不放过 blocker/major。" +
      "但每条 finding 必须给出 file:line + 证据；编造或不可达的问题会被辩方戳穿，反而损害你的可信度。",
  },
  {
    id: "defender",
    name: "辩方",
    stance:
      "你要基于真实代码为 PR 辩护：判断控方担忧是否已被现有代码/测试/上下文覆盖、是否不可达、是否属于本 PR 范围外。" +
      "不要为辩护而无视真 bug——放过 blocker/major 会被决策方直接追责。",
  },
];

const GUIDELINES = `
## Bong Review 准则

- 输出中文，结论要可核验，问题必须带 file:line。
- 代码质量从严：实现应简单直接，注释只解释非显然决策；空泛注释、过度抽象、功能蔓延都算风险。
- Plan PR 必须确认“是否符合 plan 原意”，不是只看是否改了文件。要对照 plan 的目标、阶段交付物、测试声明和跨端契约。
- 重点抓断链：新增 struct/fn/component/enum/registry/event/payload/schema 后，全仓是否有真实调用方、消费方或加载路径。
- 灵气守恒：真元/灵气流动必须走 qi_physics ledger；自写衰减/逸散/半衰常数是红旗。
- 世界观：六境界为醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚；骨币是唯一真货币。
- 测试要锁契约：happy path、边界、错误分支、状态转换；schema/enum/状态机需要 pin 测试。
- 不确定时从严；不要为了凑共识而让不明风险通过。
`.trim();

// ── 纯逻辑：测试覆盖这些函数 ────────────────────────────────────────────────
export function intEnv(name, fallback, min = Number.MIN_SAFE_INTEGER) {
  const n = parseInt(process.env[name] || "", 10);
  return Number.isFinite(n) ? Math.max(min, n) : fallback;
}

// 硬并发限制：把请求的并发数钳到 [1, hardMax]。非法输入回落到 1，绝不放大。
export function clampConcurrency(requested, hardMax) {
  const cap = Number.isFinite(hardMax) && hardMax >= 1 ? Math.floor(hardMax) : 1;
  const n = Number.isFinite(requested) ? Math.floor(requested) : 1;
  return Math.max(1, Math.min(cap, n));
}

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

// claude --output-format json 的信封解析：从 stdout 里挑出 {"type":"result",...} 那条，返回 .result 文本。
export function parseClaudeEnvelope(stdout) {
  const text = String(stdout || "");
  const lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
  for (let i = lines.length - 1; i >= 0; i--) {
    if (!lines[i].startsWith("{")) continue;
    try {
      const obj = JSON.parse(lines[i]);
      if (obj && typeof obj === "object" && (obj.type === "result" || typeof obj.result === "string")) {
        return { ok: obj.is_error !== true && obj.subtype !== "error_max_turns", result: String(obj.result ?? ""), envelope: obj };
      }
    } catch {
      /* try earlier line */
    }
  }
  // 兜底：整块 stdout 里抓一个 JSON 对象。
  const obj = extractJSON(text);
  if (obj && typeof obj.result === "string") {
    return { ok: obj.is_error !== true, result: obj.result, envelope: obj };
  }
  return { ok: false, result: "", envelope: null };
}

export function normalizeVote(v) {
  return String(v || "").toUpperCase() === "APPROVE" ? "APPROVE" : "REQUEST_CHANGES";
}

export function normalizePlanStatus(v) {
  const s = String(v || "").toLowerCase();
  if (["aligned", "misaligned", "not_plan", "unclear"].includes(s)) return s;
  return "unclear";
}

export function normalizeSeverity(value) {
  const s = String(value || "").toLowerCase();
  return ["blocker", "major", "minor"].includes(s) ? s : "minor";
}

export function severityRank(value) {
  return { blocker: 0, major: 1, minor: 2 }[normalizeSeverity(value)] ?? 3;
}

// reviewer(维度 × 角色) 的输出归一化；无法解析时按“该角色执行失败 → 从严”兜底。
export function normalizeReviewerResult(raw, dimension, role) {
  const parsed = extractJSON(raw);
  const base = { dimension: dimension.id, dimensionName: dimension.name, role: role.id, roleName: role.name };
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return {
      ...base,
      vote: "REQUEST_CHANGES",
      confidence: 0,
      plan_intent: { status: "unclear", reason: `${dimension.name}·${role.name} 输出无法解析为 JSON`, missing: [] },
      summary: `${dimension.name}·${role.name} 输出无法解析，按未通过处理。`,
      parseFailed: true,
      findings: [
        {
          severity: "major",
          file: ".github/scripts/review-claude.mjs",
          line: "0",
          title: `${dimension.name}·${role.name} 输出无法解析`,
          evidence: truncate(String(raw || ""), 800),
          recommendation: "重新触发 /review；若反复出现，检查 claude CLI 输出稳定性。",
        },
      ],
    };
  }
  return {
    ...base,
    vote: normalizeVote(parsed.vote),
    confidence: clampInt(parsed.confidence, 0, 100, 0),
    plan_intent: {
      status: normalizePlanStatus(parsed.plan_intent?.status),
      reason: truncate(String(parsed.plan_intent?.reason || ""), 1000),
      missing: normalizeStringArray(parsed.plan_intent?.missing).slice(0, 10),
    },
    summary: truncate(String(parsed.summary || ""), 1500),
    parseFailed: false,
    findings: normalizeFindings(parsed.findings),
  };
}

// 决策 agent(sol/ultra) 的最终裁决归一化。
export function normalizeDecision(raw) {
  const parsed = extractJSON(raw);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { ok: false, vote: "REQUEST_CHANGES", confidence: 0, plan_status: "unclear", rationale: "决策 agent 输出无法解析", blocking: [] };
  }
  return {
    ok: true,
    vote: normalizeVote(parsed.vote),
    confidence: clampInt(parsed.confidence, 0, 100, 0),
    plan_status: normalizePlanStatus(parsed.plan_intent?.status ?? parsed.plan_status),
    rationale: truncate(String(parsed.rationale || parsed.summary || ""), 2000),
    blocking: normalizeFindings(parsed.blocking).slice(0, 20),
  };
}

// 维度 A(Plan 原意) 是否被两方都确认 aligned。
export function planAlignedFromDimensions(dimResults) {
  const a = dimResults.find((d) => d.id === "A");
  if (!a || !a.opinions?.length) return false;
  return a.opinions.every((o) => o.plan_intent?.status === "aligned");
}

// 最终 gate：以决策 agent 为准，叠加确定性安全网(plan 未 aligned / 决策失败回落到 finding 计数)。
export function deriveGate(decision, dimResults, hasPlan) {
  const merged = mergeFindings(dimResults.flatMap((d) => d.opinions || []));
  const blockers = merged.filter((f) => severityRank(f.severity) <= 1); // blocker/major
  const planOk = hasPlan ? planAlignedFromDimensions(dimResults) : true;

  let passed;
  let reason;
  if (decision.ok) {
    passed = decision.vote === "APPROVE";
    reason = decision.rationale || (passed ? "决策 agent 判定可合并。" : "决策 agent 判定需修改。");
  } else {
    // 决策 agent 失联 → 回落确定性规则：无 blocker/major 且 plan 对齐才通过。
    passed = blockers.length === 0 && planOk;
    reason = `决策 agent 未产出有效裁决，回落确定性判定：blocker/major=${blockers.length}，plan 对齐=${planOk}。`;
  }

  if (hasPlan && !planOk) {
    passed = false;
    reason = `${reason}${reason ? " " : ""}Plan 原意未被双方确认为 aligned，强制 REQUEST_CHANGES。`.trim();
  }

  return {
    passed,
    status: passed ? "APPROVED" : "REQUEST_CHANGES",
    reason,
    blockerCount: blockers.length,
    planOk,
    label: passed ? "通过" : "要求修改",
  };
}

export function findPlanName(meta) {
  const haystack = `${meta.title || ""} ${meta.headRefName || ""} ${meta.body || ""}`;
  const textMatch = haystack.match(/plan-[a-z0-9-]+-v\d+/i);
  if (textMatch) return textMatch[0];

  for (const file of meta.files || []) {
    const path = String(file.path || "");
    const pathMatch = path.match(/(?:^|\/)(plan-[a-z0-9-]+-v\d+)\.md$/i);
    if (pathMatch) return pathMatch[1];
  }
  return null;
}

export function mergeFindings(results) {
  const byKey = new Map();
  for (const result of results) {
    const tag = result.dimension ? `${result.dimension}/${result.role || ""}` : result.role || result.reviewer || "?";
    for (const finding of result.findings || []) {
      const file = String(finding.file || "").trim();
      const line = String(finding.line || "").trim();
      const title = String(finding.title || "").trim();
      if (!file && !title) continue;
      const key = `${file}|${line}|${title.toLowerCase().replace(/\s+/g, " ")}`;
      const current = byKey.get(key);
      if (!current) {
        byKey.set(key, { ...finding, file, line, title, reviewers: [tag] });
        continue;
      }
      if (!current.reviewers.includes(tag)) current.reviewers.push(tag);
      if (severityRank(finding.severity) < severityRank(current.severity)) current.severity = finding.severity;
      if (String(finding.evidence || "").length > String(current.evidence || "").length) current.evidence = finding.evidence;
      if (String(finding.recommendation || "").length > String(current.recommendation || "").length) {
        current.recommendation = finding.recommendation;
      }
    }
  }
  return [...byKey.values()].sort(
    (a, b) => severityRank(a.severity) - severityRank(b.severity) || b.reviewers.length - a.reviewers.length,
  );
}

// ── 主流程 ───────────────────────────────────────────────────────────────────
async function main() {
  if (!PR || !/^\d+$/.test(String(PR))) {
    console.error("PR_NUMBER 未设置或非法，必须是纯数字。");
    process.exit(1);
  }
  if (!process.env.ANTHROPIC_API_KEY && !process.env.ANTHROPIC_AUTH_TOKEN) {
    console.error("缺 ANTHROPIC_API_KEY / ANTHROPIC_AUTH_TOKEN。");
    process.exit(1);
  }

  const context = loadPrContext(PR);
  console.error(
    `Review v4(claude): PR #${PR} · ${context.changedLines} 行/${context.changedFiles} 文件 · reviewer=${MODEL_REVIEWER} 决策=${MODEL_DECISION} · 并发上限=${CONCURRENCY}(硬顶 ${HARD_MAX_CONCURRENCY})`,
  );

  // 维度串行：一次只审一个维度。
  const dimResults = [];
  for (const dimension of DIMENSIONS) {
    console.error(`── 维度 ${dimension.id} ${dimension.name} ──`);
    // 第一轮：控方 / 辩方并行独立审。
    const initial = await mapLimit(ROLES, CONCURRENCY, (role) =>
      runClaude(initialPrompt(context, dimension, role), { model: MODEL_REVIEWER, label: `${dimension.id}-${role.id}-initial` }).then(
        (raw) => normalizeReviewerResult(raw, dimension, role),
      ),
    );
    // 第二轮：互看对方观点，博弈复投。
    const debateContext = initial.map(compactReviewer);
    const opinions = await mapLimit(ROLES, CONCURRENCY, (role) => {
      const peer = debateContext.find((r) => r.role !== role.id) || null;
      return runClaude(debatePrompt(context, dimension, role, peer), {
        model: MODEL_REVIEWER,
        label: `${dimension.id}-${role.id}-debate`,
      }).then((raw) => normalizeReviewerResult(raw, dimension, role));
    });
    dimResults.push({ id: dimension.id, name: dimension.name, initial, opinions });
  }

  // 唯一决策步：sol + ultra thinking，读全部维度结论后给最终裁决。
  console.error(`── 决策 ${MODEL_DECISION} (ultra thinking ${DECISION_THINKING_TOKENS}) ──`);
  const decisionRaw = await runClaude(decisionPrompt(context, dimResults), {
    model: MODEL_DECISION,
    label: "decision",
    thinkingTokens: DECISION_THINKING_TOKENS,
  });
  const decision = normalizeDecision(decisionRaw);

  const gate = deriveGate(decision, dimResults, Boolean(context.plan));
  const body = renderComment(context, dimResults, decision, gate);
  writeFileSync("/tmp/review.md", body);

  if (DRY_RUN) {
    console.log(body);
  } else {
    gh(["pr", "comment", PR, "--body-file", "/tmp/review.md"]);
    console.error("已发布 review 评论。");
  }

  if (FAIL_ON_GATE && !gate.passed) {
    console.error(`Review gate 未通过：${gate.status} · ${gate.reason}`);
    process.exit(1);
  }
}

function loadPrContext(pr) {
  const meta = JSON.parse(gh(["pr", "view", pr, "--json", "title,body,headRefName,files"]));
  let diff = gh(["pr", "diff", pr]);
  let diffTruncated = false;
  if (diff.length > MAX_DIFF) {
    diff = diff.slice(0, MAX_DIFF);
    diffTruncated = true;
  }

  const changedFiles = (meta.files || []).length;
  const changedLines = (meta.files || []).reduce((sum, file) => sum + (file.additions || 0) + (file.deletions || 0), 0);
  const fileList = (meta.files || []).map((f) => `- ${f.path} (+${f.additions}/-${f.deletions})`).join("\n");
  const plan = findPlan(meta);

  return {
    pr,
    title: meta.title || "",
    body: truncate(meta.body || "", 5000),
    headRefName: meta.headRefName || "",
    fileList,
    changedFiles,
    changedLines,
    diff,
    diffTruncated,
    plan,
  };
}

function findPlan(meta) {
  const name = findPlanName(meta);
  if (!name) return null;
  for (const dir of ["docs", "docs/finished_plans", "docs/plans-skeleton"]) {
    const path = `${dir}/${name}.md`;
    if (existsSync(path)) {
      return { name, path, text: truncate(readFileSync(path, "utf8"), MAX_PLAN) };
    }
  }
  return { name, path: null, text: "" };
}

const REVIEWER_JSON_SHAPE = `只输出 JSON，不要 markdown，不要前言：
{
  "dimension": "维度字母",
  "role": "prosecutor|defender",
  "vote": "APPROVE|REQUEST_CHANGES",
  "confidence": 0-100,
  "plan_intent": {
    "status": "aligned|misaligned|not_plan|unclear",
    "reason": "仅维度 A 需认真填；其它维度可标 not_plan",
    "missing": ["若不符合或无法确认，列缺口"]
  },
  "summary": "一句到三句总结",
  "findings": [
    {
      "severity": "blocker|major|minor",
      "file": "路径",
      "line": "行号或范围",
      "title": "一句话问题",
      "evidence": "关键证据，简短引用或转述",
      "recommendation": "修复建议"
    }
  ]
}`;

function initialPrompt(context, dimension, role) {
  return `
你是 Bong PR review 面板中【维度 ${dimension.id}：${dimension.name}】的【${role.name}】。
本维度重点：${dimension.focus}
你的立场：${role.stance}

${GUIDELINES}

请审查 PR #${context.pr}，只聚焦本维度。你可以用只读工具(Read/Grep/Glob)打开仓库真实文件、grep 调用方、核对 plan 和测试。
不要修改文件。不要给泛泛建议；只报可核验问题。

${contextBlock(context)}

${REVIEWER_JSON_SHAPE}
`.trim();
}

function debatePrompt(context, dimension, role, peer) {
  return `
你是【维度 ${dimension.id}：${dimension.name}】的【${role.name}】，已完成首轮，现进入与对方的博弈复投。
你的立场：${role.stance}
请阅读对方(${peer ? peer.roleName : "对方"})的首轮意见，独立判断是否调整结论。不要为了制造共识而妥协，也不要固执己见无视对方的有效反驳。

${GUIDELINES}

## 对方首轮意见
${peer ? JSON.stringify(peer, null, 2) : "(对方无有效输出)"}

${contextBlock(context, { includeDiff: false })}

${REVIEWER_JSON_SHAPE}
`.trim();
}

function decisionPrompt(context, dimResults) {
  const digest = dimResults.map((d) => ({
    dimension: d.id,
    name: d.name,
    opinions: (d.opinions || []).map(compactReviewer),
  }));
  const planLine = context.plan
    ? `本 PR 关联 plan \`${context.plan.name}\`${context.plan.path ? ` (${context.plan.path})` : "（仓库未找到文件）"}；只有真正符合 plan 原意和交付物才可通过。`
    : "本 PR 未检测到关联 plan。";
  return `
ultrathink。你是 Bong PR review 面板的【最终决策方】。四个维度(Plan 原意 / 运行接线 / 正确性与守恒 / 代码质量与测试)已各自经过控方 / 辩方两轮博弈，结论如下。
你的职责：综合四维博弈结果，独立复核关键分歧点(可用只读工具核对真实代码)，给出唯一的最终裁决。

通过标准：
- 若关联 plan，必须真正符合 plan 原意和交付物。
- 不存在 blocker/major 的正确性、断链、守恒、schema、测试或代码质量问题。
- 控辩双方对某 finding 有分歧时，以真实代码为准；不确定就从严 REQUEST_CHANGES。

${GUIDELINES}

## PR
#${context.pr} ${context.title}
${planLine}

## 变更文件 (${context.changedFiles} 个，${context.changedLines} 行)
${context.fileList || "(无)"}

## 四维博弈结论
${JSON.stringify(digest, null, 2)}

只输出 JSON，不要 markdown，不要前言：
{
  "vote": "APPROVE|REQUEST_CHANGES",
  "confidence": 0-100,
  "plan_intent": { "status": "aligned|misaligned|not_plan|unclear", "reason": "plan 原意最终确认" },
  "rationale": "最终裁决理由，点名采纳/驳回了哪些控辩分歧",
  "blocking": [
    { "severity": "blocker|major|minor", "file": "路径", "line": "行号", "title": "阻塞问题", "recommendation": "修复建议" }
  ]
}
`.trim();
}

function contextBlock(context, { includeDiff = true } = {}) {
  const planBlock = context.plan
    ? context.plan.text
      ? `## 关联 Plan：${context.plan.name} (${context.plan.path})\n${context.plan.text}`
      : `## 关联 Plan：${context.plan.name}\n仓库内未找到 plan 文件，请按无法完整确认处理。`
    : "## 关联 Plan\n未检测到 plan 名称；plan 原意项标 not_plan。";
  const diffBlock = includeDiff
    ? `## Diff${context.diffTruncated ? `（已截断到 ${MAX_DIFF} 字符）` : ""}
\`\`\`diff
${context.diff}
\`\`\``
    : "## Diff\n复投阶段不重复粘贴完整 diff；请结合对方意见、文件列表和只读工具核对真实仓库。";

  return `
## PR
#${context.pr} ${context.title}

## PR Body
${context.body || "(空)"}

## 变更文件 (${context.changedFiles} 个，${context.changedLines} 行)
${context.fileList || "(无)"}

${planBlock}

${diffBlock}
`.trim();
}

function compactReviewer(result) {
  return {
    dimension: result.dimension,
    role: result.role,
    roleName: result.roleName,
    vote: result.vote,
    confidence: result.confidence,
    plan_intent: result.plan_intent,
    summary: result.summary,
    findings: (result.findings || []).map((f) => ({
      severity: f.severity,
      file: f.file,
      line: f.line,
      title: f.title,
      evidence: f.evidence,
    })),
  };
}

// ── claude CLI 调用（全局硬并发信号量）────────────────────────────────────────
let activeSlots = 0;
const slotWaiters = [];
async function withSlot(fn) {
  if (activeSlots >= CONCURRENCY) {
    await new Promise((resolve) => slotWaiters.push(resolve));
  }
  activeSlots++;
  try {
    return await fn();
  } finally {
    activeSlots--;
    const next = slotWaiters.shift();
    if (next) next();
  }
}

async function runClaude(prompt, { model, label, thinkingTokens = 0 }) {
  return withSlot(async () => {
    const tmp = mkdtempSync(join(tmpdir(), `bong-rev-${label}-`));
    const args = [
      "-p",
      "--model",
      model,
      "--output-format",
      "json",
      // CI 是一次性克隆，工具不设限：reviewer 可用 Bash grep 调用方 / 跑 cargo 深挖。
      "--allowedTools",
      "Read,Grep,Glob,Bash",
      "--dangerously-skip-permissions",
    ];
    console.error(`▶ claude ${label} (${model})`);
    try {
      const result = await spawnClaude(args, prompt, CLAUDE_TIMEOUT_MS, thinkingTokens);
      const env = parseClaudeEnvelope(result.stdout);
      if (env.result.trim()) {
        if (result.code !== 0 || !env.ok) {
          console.error(`  claude ${label} exit=${result.code} ok=${env.ok}，但已产出 result，继续解析`);
        }
        return env.result;
      }
      const failure = claudeFailureText(result);
      console.error(`  claude ${label} failed: ${failure}`);
      return JSON.stringify({
        vote: "REQUEST_CHANGES",
        confidence: 0,
        plan_intent: { status: "unclear", reason: `claude ${label} 执行失败`, missing: [] },
        summary: `claude ${label} 执行失败：${failure}`,
        findings: [
          {
            severity: "major",
            file: ".github/scripts/review-claude.mjs",
            line: "0",
            title: `claude reviewer ${label} 执行失败`,
            evidence: failure,
            recommendation: "检查 claude CLI、ANTHROPIC_BASE_URL / ANTHROPIC_API_KEY 后重新触发 /review。",
          },
        ],
      });
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
}

function spawnClaude(args, stdin, timeoutMs, thinkingTokens) {
  return new Promise((resolve) => {
    const env = { ...process.env };
    // 只有决策步开 ultra thinking；reviewer 不设 → 走性价比快速路径。
    if (thinkingTokens > 0) env.MAX_THINKING_TOKENS = String(thinkingTokens);
    else delete env.MAX_THINKING_TOKENS;

    const child = spawn("claude", args, { env, stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, timeoutMs);

    child.stdout.on("data", (chunk) => {
      stdout = appendCap(stdout, chunk);
    });
    child.stderr.on("data", (chunk) => {
      stderr = appendCap(stderr, chunk);
    });
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      resolve({ code: timedOut ? 124 : code, signal, stdout, stderr });
    });
    child.stdin.end(stdin);
  });
}

async function mapLimit(items, limit, fn) {
  const out = new Array(items.length);
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) {
      const idx = next++;
      out[idx] = await fn(items[idx], idx);
    }
  });
  await Promise.all(workers);
  return out;
}

export function claudeFailureText(result) {
  const stderr = excerptLog(result.stderr || "", 2000);
  const stdout = excerptLog(result.stdout || "", 1200);
  return [
    `exit=${result.code}`,
    result.signal ? `signal=${result.signal}` : "",
    stderr ? `stderr: ${stderr}` : "",
    stdout ? `stdout: ${stdout}` : "",
  ]
    .filter(Boolean)
    .join(" | ");
}

export function excerptLog(value, limit) {
  const text = String(value || "").trim();
  if (text.length <= limit) return text;
  const head = Math.floor(limit * 0.45);
  const tail = Math.max(200, limit - head - 80);
  return `${text.slice(0, head)}\n...[truncated ${text.length - head - tail} chars]...\n${text.slice(text.length - tail)}`;
}

function renderComment(context, dimResults, decision, gate) {
  const findings = mergeFindings(dimResults.flatMap((d) => d.opinions || []));
  const dimRows = dimResults
    .map((d) => {
      const votes = (d.opinions || []).map((o) => `${o.roleName}:${o.vote === "APPROVE" ? "✅" : "❌"}`).join(" ");
      const n = (d.opinions || []).reduce((s, o) => s + (o.findings?.length || 0), 0);
      return `| ${d.id} ${d.name} | ${votes || "-"} | ${n} |`;
    })
    .join("\n");
  const findingRows = findings.length
    ? findings
        .map(
          (f) =>
            `| ${f.severity} | ${f.file}:${f.line || "?"} | ${escapeCell(f.title)} | ${f.reviewers.join(",")} | ${escapeCell(f.recommendation || "")} |`,
        )
        .join("\n")
    : "| - | - | 控辩双方未发现高置信度阻塞问题 | - | - |";

  const passLine = gate.passed ? "✅ **通过**" : "❌ **未通过**";
  const planLine = context.plan
    ? `> Plan：\`${context.plan.name}\`${context.plan.path ? ` (${context.plan.path})` : "（未找到文件）"} · 双方确认 aligned=${gate.planOk}`
    : "> Plan：未检测到 plan";
  const detail = JSON.stringify(
    dimResults.map((d) => ({ dimension: d.id, name: d.name, opinions: (d.opinions || []).map(compactReviewer) })),
    null,
    2,
  );

  const body = `
## 🔭 Review · PR #${context.pr}

${passLine}：${gate.status} · ${escapeInline(gate.reason)}

> 引擎：Claude Code(claude CLI)，reviewer 模型 \`${MODEL_REVIEWER}\`(性价比)，决策模型 \`${MODEL_DECISION}\`(ultra thinking)，base_url \`${process.env.ANTHROPIC_BASE_URL || "https://api.shuaiapi.com"}\`。
> 结构：4 维度串行 × 控辩并行博弈(initial+debate)，硬并发上限 ${CONCURRENCY}(编译期顶 ${HARD_MAX_CONCURRENCY})。
> 触发：PR 首次创建自动跑；后续提交不自动跑，需要评论 \`/review\` 复审。
${planLine}
${context.diffTruncated ? `> Diff 已截断至 ${MAX_DIFF} 字符，reviewer 可继续用只读工具查仓库。` : ""}

**🧑‍⚖️ 决策裁决**（\`${MODEL_DECISION}\` · confidence ${decision.confidence}）

${escapeInline(decision.rationale || "(无)")}

**📊 四维博弈**

| 维度 | 控辩投票 | Findings |
|---|---|---:|
${dimRows}

**🔎 Findings**

| 严重度 | 位置 | 问题 | 来源 | 建议 |
|---|---|---|---|---|
${findingRows}

<details>
<summary>四维博弈原始结构化摘要</summary>

\`\`\`json
${truncate(detail, 48_000)}
\`\`\`
</details>
`.trim();

  return truncate(body, 64_000);
}

function gh(args) {
  return execFileSync("gh", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function normalizeFindings(value) {
  if (!Array.isArray(value)) return [];
  return value
    .filter((f) => f && typeof f === "object")
    .map((f) => ({
      severity: normalizeSeverity(f.severity),
      file: truncate(String(f.file || ""), 300),
      line: truncate(String(f.line || ""), 80),
      title: truncate(String(f.title || ""), 300),
      evidence: truncate(String(f.evidence || ""), 1200),
      recommendation: truncate(String(f.recommendation || ""), 800),
    }))
    .filter((f) => f.title || f.file)
    .slice(0, 20);
}

function normalizeStringArray(value) {
  if (!Array.isArray(value)) return [];
  return value.map((v) => truncate(String(v || ""), 300)).filter(Boolean);
}

function clampInt(value, min, max, fallback) {
  const n = typeof value === "string" ? parseInt(value, 10) : value;
  if (!Number.isFinite(n)) return fallback;
  return Math.max(min, Math.min(max, Math.round(n)));
}

function truncate(value, limit) {
  const s = String(value || "");
  return s.length <= limit ? s : `${s.slice(0, Math.max(0, limit - 20))}\n...[truncated]`;
}

function appendCap(current, chunk, limit = 200_000) {
  const next = current + chunk.toString("utf8");
  return next.length > limit ? next.slice(next.length - limit) : next;
}

function escapeCell(value) {
  return truncate(String(value || ""), 500).replace(/\n/g, "<br>").replace(/\|/g, "\\|");
}

function escapeInline(value) {
  return truncate(String(value || ""), 2000).replace(/\n+/g, " ");
}

const isEntry = import.meta.url === `file://${process.argv[1]}`;
if (isEntry) {
  main().catch((error) => {
    console.error(error);
    process.exit(1);
  });
}
