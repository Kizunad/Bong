#!/usr/bin/env node
// Review v3 —— 4 个 Codex gpt-5.6-sol high reviewer 对同一 PR 做博弈式审核。
//
// 流程：
// 1. 拉取 PR metadata / diff / 关联 plan。
// 2. 4 个 reviewer 独立审查，分别盯 plan 原意、运行接线、正确性、代码质量。
// 3. 把首轮意见互相公开，4 个 reviewer 复投最终票。
// 4. 只有 3/1 或 4/0 APPROVE 才通过；2/2 直接视为未通过。
//
// 依赖：Node 内置模块 + gh + codex CLI。无 npm runtime dependency。

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PR = process.env.PR_NUMBER;
const MODEL = process.env.REVIEW_CODEX_MODEL || "gpt-5.6-sol";
const MAX_DIFF = intEnv("REVIEW_MAX_DIFF", 40_000, 10_000);
const MAX_PLAN = intEnv("REVIEW_MAX_PLAN", 20_000, 5_000);
const CODEX_TIMEOUT_MS = intEnv("REVIEW_CODEX_TIMEOUT_MS", 900_000, 120_000);
const REVIEW_TOTAL_TIMEOUT_MS = intEnv("REVIEW_TOTAL_TIMEOUT_MINUTES", 35, 5) * 60_000;
const GH_TIMEOUT_MS = intEnv("REVIEW_GH_TIMEOUT_MS", 30_000, 1_000);
const CODEX_CONCURRENCY = intEnv("REVIEW_CODEX_CONCURRENCY", 1, 1);
const CODEX_RETRIES = intEnv("REVIEW_CODEX_RETRIES", 3, 1);
const CODEX_RETRY_MS = intEnv("REVIEW_CODEX_RETRY_MS", 15_000, 1_000);
const DRY_RUN = /^(1|true|yes)$/i.test(String(process.env.REVIEW_DRY_RUN || "").trim());
const FAIL_ON_GATE = process.env.REVIEW_FAIL_ON_GATE !== "0";
const CIRCUIT_MARKER = "bong-review-circuit";
const HANDOFF_MARKER = "bong-review-handoff";
const CIRCUIT_STATE_MARKER = "<!-- bong-review-circuit-state:v1 -->";
const CIRCUIT_STATE_TITLE = "[automation] Review infrastructure circuit state";
const CIRCUIT_THRESHOLD = intEnv("REVIEW_INFRA_FAILURE_THRESHOLD", 3, 1);
const CIRCUIT_WINDOW_MS = intEnv("REVIEW_INFRA_FAILURE_WINDOW_MINUTES", 60, 1) * 60_000;
const CIRCUIT_DURATION_MS = intEnv("REVIEW_CIRCUIT_MINUTES", 60, 1) * 60_000;
const OUTCOME_FILE = process.env.REVIEW_OUTCOME_FILE || "/tmp/review-run-outcome.json";
let reviewDeadlineMs = Number.POSITIVE_INFINITY;

const REVIEWERS = [
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

const GUIDELINES = `
## Bong Review 准则

- 输出中文，结论要可核验，问题必须带 file:line。
- 代码质量从严：实现应简单直接，注释只解释非显然决策；空泛注释、过度抽象、功能蔓延都算风险。
- Plan PR 必须确认“是否符合 plan 原意”，不是只看是否改了文件。要对照 plan 的目标、阶段交付物、测试声明和跨端契约。
- 重点抓断链：新增 struct/fn/component/enum/registry/event/payload/schema 后，全仓是否有真实调用方、消费方或加载路径。
- 灵气守恒：真元/灵气流动必须走 qi_physics ledger；自写衰减/逸散/半衰常数是红旗。
- 世界观：六境界为醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚；骨币是唯一真货币。
- 测试要锁契约：happy path、边界、错误分支、状态转换；schema/enum/状态机需要 pin 测试。
- 不确定时投 REQUEST_CHANGES；不要为了凑多数而让不明风险通过。
`.trim();

// ── 纯逻辑：测试覆盖这些函数 ────────────────────────────────────────────────
export function intEnv(name, fallback, min = Number.MIN_SAFE_INTEGER) {
  const n = parseInt(process.env[name] || "", 10);
  return Number.isFinite(n) ? Math.max(min, n) : fallback;
}

export function isManualReviewTrigger(eventName) {
  return eventName === "issue_comment" || eventName === "workflow_dispatch";
}

export function renderHiddenMarker(name, payload) {
  const safe = JSON.stringify(payload).replaceAll("--", "——");
  return `<!-- ${name} ${safe} -->`;
}

export function parseHiddenMarkers(value, name = CIRCUIT_MARKER) {
  const bodies = Array.isArray(value) ? value : [value];
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`<!--\\s*${escaped}\\s+([^]*?)-->`, "g");
  const parsed = [];
  for (const item of bodies) {
    const body = String(item?.body ?? item ?? "");
    for (const match of body.matchAll(pattern)) {
      const marker = extractJSON(match[1]);
      if (marker && typeof marker === "object" && !Array.isArray(marker)) parsed.push(marker);
    }
  }
  return parsed;
}

export function normalizeCircuitEvent(value) {
  const runId = String(value?.run_id || "");
  if (
    value?.v !== 1 ||
    value?.kind !== "infra_failure" ||
    !/^\d+$/.test(runId) ||
    !Number.isFinite(new Date(value.at).getTime())
  ) {
    return null;
  }
  return { ...value, run_id: runId };
}

export function parseTrustedCircuitEvents(comments, trustedLogin = "github-actions[bot]") {
  const seenRuns = new Set();
  const events = [];
  for (const comment of comments || []) {
    if (comment?.user?.login !== trustedLogin || comment?.user?.type !== "Bot") continue;
    const marker = normalizeCircuitEvent(parseHiddenMarkers(comment.body)[0]);
    if (!marker || seenRuns.has(marker.run_id)) continue;
    seenRuns.add(marker.run_id);
    events.push(marker);
  }
  return events;
}

export function selectCircuitStateIssues(issues) {
  return (issues || [])
    .filter(
      (issue) =>
        !issue?.pull_request &&
        issue?.user?.login === "github-actions[bot]" &&
        issue?.user?.type === "Bot" &&
        issue?.title === CIRCUIT_STATE_TITLE &&
        String(issue.body || "").includes(CIRCUIT_STATE_MARKER),
    )
    .map((issue) => String(issue.number))
    .filter((number) => /^\d+$/.test(number))
    .sort((a, b) => Number(a) - Number(b));
}

export function resolveCircuitStateIssueNumbers(createdNumber, refreshedNumbers) {
  const refreshed = (refreshedNumbers || [])
    .map(String)
    .filter((number) => /^\d+$/.test(number))
    .sort((a, b) => Number(a) - Number(b));
  return refreshed.length ? refreshed : [String(createdNumber)];
}

export function boundedAttemptTimeout(configuredMs, remainingMs, cleanupReserveMs = 180_000) {
  return Math.max(0, Math.min(configuredMs, remainingMs - cleanupReserveMs));
}

export function evaluateCircuit(events, now, { threshold = 3, windowMs = 3_600_000, durationMs = 3_600_000 } = {}) {
  const nowMs = new Date(now).getTime();
  const failures = events
    .filter((event) => event?.kind === "infra_failure")
    .map((event) => new Date(event.at).getTime())
    .filter((at) => Number.isFinite(at) && at <= nowMs)
    .sort((a, b) => a - b);

  let openedAt = null;
  let openUntil = null;
  for (let i = threshold - 1; i < failures.length; i += 1) {
    if (failures[i] - failures[i - threshold + 1] <= windowMs) {
      const candidateUntil = failures[i] + durationMs;
      if (candidateUntil > (openUntil ?? Number.NEGATIVE_INFINITY)) {
        openedAt = failures[i];
        openUntil = candidateUntil;
      }
    }
  }

  return {
    open: openUntil !== null && nowMs < openUntil,
    openedAt: openedAt === null ? null : new Date(openedAt).toISOString(),
    openUntil: openUntil === null ? null : new Date(openUntil).toISOString(),
    failureCount: failures.filter((at) => nowMs - at <= windowMs).length,
  };
}

export function classifyReviewRun(firstRound, finalRound) {
  const results = [...(firstRound || []), ...(finalRound || [])];
  if (results.some(isCodexExecutionFailure)) return "infra_failure";
  return decideGate(finalRound || []).passed ? "passed" : "gate_failure";
}

export function classifyWorkflowFinalization({ install = "skipped", configure = "skipped", review = "skipped", outcomeKind = null }) {
  if (install === "failure") {
    return { kind: "infra_failure", phase: "cli_install", reason: "Codex CLI 安装或版本检查失败", shouldRecord: true };
  }
  if (configure === "failure" || (install === "success" && configure === "skipped")) {
    return { kind: "infra_failure", phase: "provider_config", reason: "Codex provider 配置失败或被意外跳过", shouldRecord: true };
  }
  if (review === "failure" && outcomeKind === "gate_failure") {
    return { kind: "gate_failure", shouldRecord: false };
  }
  if (review === "failure" && outcomeKind === "infra_failure") {
    return { kind: "infra_failure", shouldRecord: false };
  }
  if (review === "failure" || (review === "skipped" && configure === "success")) {
    return {
      kind: "infra_failure",
      phase: "review_process",
      reason: review === "failure" ? "Review 脚本、CLI 或沙箱异常退出，且未产出可识别结果" : "Review 步骤被意外跳过，未执行 reviewer",
      shouldRecord: true,
    };
  }
  return { kind: "passed", shouldRecord: false };
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

export function normalizeVote(v) {
  return String(v || "").toUpperCase() === "APPROVE" ? "APPROVE" : "REQUEST_CHANGES";
}

export function normalizePlanStatus(v) {
  const s = String(v || "").toLowerCase();
  if (["aligned", "misaligned", "not_plan", "unclear"].includes(s)) return s;
  return "unclear";
}

export function normalizeResult(raw, reviewer) {
  const parsed = extractJSON(raw);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return {
      reviewer: reviewer.id,
      name: reviewer.name,
      vote: "REQUEST_CHANGES",
      confidence: 0,
      plan_intent: { status: "unclear", reason: "reviewer 输出无法解析为 JSON", missing: [] },
      summary: "reviewer 输出无法解析，按未通过处理。",
      findings: [
        {
          severity: "major",
          file: ".github/scripts/review.mjs",
          line: "0",
          title: `${reviewer.name} 输出无法解析`,
          evidence: truncate(String(raw || ""), 800),
          recommendation: "重新触发 /review；若反复出现，检查 Codex 输出稳定性。",
        },
      ],
    };
  }

  return {
    reviewer: String(parsed.reviewer || reviewer.id),
    name: reviewer.name,
    vote: normalizeVote(parsed.vote),
    confidence: clampInt(parsed.confidence, 0, 100, 0),
    plan_intent: {
      status: normalizePlanStatus(parsed.plan_intent?.status),
      reason: truncate(String(parsed.plan_intent?.reason || ""), 1000),
      missing: normalizeStringArray(parsed.plan_intent?.missing).slice(0, 10),
    },
    summary: truncate(String(parsed.summary || ""), 1500),
    findings: normalizeFindings(parsed.findings),
  };
}

export function decideGate(results) {
  const approve = results.filter((r) => r.vote === "APPROVE").length;
  const request = results.length - approve;
  if (approve === request) {
    return { passed: false, status: "TIE", approve, request, label: `${approve}/${request} 平票，未通过` };
  }
  if (approve >= 3) {
    return { passed: true, status: "APPROVED", approve, request, label: `${approve}/${request} 通过` };
  }
  return { passed: false, status: "REQUEST_CHANGES", approve, request, label: `${approve}/${request} 要求修改` };
}

export function applyPlanIntentGate(results, hasPlan) {
  if (!hasPlan) return results;
  return results.map((result) => {
    if (result.plan_intent?.status === "aligned") return result;
    const reason = result.plan_intent?.reason || "未确认符合 plan 原意";
    return {
      ...result,
      vote: "REQUEST_CHANGES",
      summary: `${result.summary || ""}${result.summary ? " " : ""}Plan 原意未确认：${reason}`.trim(),
    };
  });
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
    for (const finding of result.findings || []) {
      const file = String(finding.file || "").trim();
      const line = String(finding.line || "").trim();
      const title = String(finding.title || "").trim();
      if (!file && !title) continue;
      const key = `${file}|${line}|${title.toLowerCase().replace(/\s+/g, " ")}`;
      const current = byKey.get(key);
      if (!current) {
        byKey.set(key, {
          ...finding,
          file,
          line,
          title,
          reviewers: [result.reviewer],
        });
        continue;
      }
      current.reviewers.push(result.reviewer);
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
  const command = process.argv[2] || "review";
  if (command === "circuit-preflight") return circuitPreflight();
  if (command === "workflow-finalize") return workflowFinalize();
  return runReview();
}

async function runReview() {
  requirePrNumber();
  reviewDeadlineMs = Date.now() + REVIEW_TOTAL_TIMEOUT_MS;
  if (!process.env.REVIEW_CODEX_API_KEY && !process.env.OPENAI_API_KEY) {
    await recordInfrastructureFailure("provider_config", "缺 REVIEW_CODEX_API_KEY / OPENAI_API_KEY");
    writeOutcome("infra_failure");
    return 0;
  }

  const context = loadPrContext(PR);
  console.error(`Review v3: PR #${PR} · ${context.changedLines} 行/${context.changedFiles} 文件 · 4×${MODEL} high`);

  const firstRound = await mapLimit(REVIEWERS, CODEX_CONCURRENCY, (reviewer) =>
    runCodex(initialPrompt(context, reviewer), `initial-${reviewer.id}`).then((raw) => normalizeResult(raw, reviewer)),
  );

  const debateContext = firstRound.map(compactResultForPrompt);
  const finalRoundRaw = firstRound.every(isCodexExecutionFailure)
    ? firstRound
    : await mapLimit(REVIEWERS, CODEX_CONCURRENCY, (reviewer) =>
        runCodex(finalPrompt(context, reviewer, debateContext), `final-${reviewer.id}`).then((raw) => normalizeResult(raw, reviewer)),
      );
  if (finalRoundRaw === firstRound) {
    console.error("首轮 Codex reviewer 全部执行失败，跳过复投以保留原始失败诊断。");
  }
  const finalRound = applyPlanIntentGate(finalRoundRaw, Boolean(context.plan));
  const outcome = classifyReviewRun(firstRound, finalRound);
  if (outcome === "infra_failure") {
    const failed = [...firstRound, ...finalRound].filter(isCodexExecutionFailure);
    const reason = failed.map((result) => result.summary).filter(Boolean).join(" | ");
    await recordInfrastructureFailure("reviewer_execution", reason || "Codex reviewer 执行失败");
    writeOutcome("infra_failure");
    return 0;
  }

  const gate = decideGate(finalRound);
  const body = renderComment(context, firstRound, finalRound, gate);
  writeFileSync("/tmp/review.md", body);

  if (DRY_RUN) {
    console.log(body);
  } else {
    gh(["pr", "comment", PR, "--body-file", "/tmp/review.md"]);
    console.error("已发布 review 评论。");
  }

  writeOutcome(gate.passed ? "passed" : "gate_failure", { gate: gate.status });
  if (FAIL_ON_GATE && !gate.passed) {
    console.error(`Review gate 未通过：${gate.label}`);
    return 1;
  }
  return 0;
}

async function circuitPreflight() {
  requirePrNumber();
  const trigger = process.env.REVIEW_TRIGGER || process.env.GITHUB_EVENT_NAME || "";
  if (isManualReviewTrigger(trigger)) {
    console.error(`手动触发 ${trigger} 旁路 Review 熔断。`);
    setOutput("should_run", "true");
    return 0;
  }

  let state;
  try {
    state = currentCircuitState(loadCircuitEvents());
  } catch (error) {
    console.error(`::warning::读取 Review 熔断状态失败，按 fail-open 继续执行：${errorText(error)}`);
  }
  if (state?.open) {
    const body = renderCircuitSkipComment(state);
    try {
      if (DRY_RUN) console.log(body);
      else postIssueComment(PR, body);
    } catch (error) {
      console.error(`::warning::发布 Review 熔断跳过评论失败：${errorText(error)}`);
    }
    writeOutcome("circuit_skipped", { openUntil: state.openUntil });
    setOutput("should_run", "false");
    console.error(`Review 自动触发已熔断跳过，截止 ${state.openUntil}。`);
    return 0;
  }

  setOutput("should_run", "true");
  return 0;
}

async function workflowFinalize() {
  requirePrNumber();
  const decision = classifyWorkflowFinalization({
    install: process.env.REVIEW_INSTALL_OUTCOME || "skipped",
    configure: process.env.REVIEW_CONFIGURE_OUTCOME || "skipped",
    review: process.env.REVIEW_STEP_OUTCOME || "skipped",
    outcomeKind: readOutcome()?.kind || null,
  });

  if (decision.kind === "gate_failure") return 1;
  if (decision.kind === "infra_failure" && decision.shouldRecord) {
    await recordInfrastructureFailure(decision.phase, decision.reason);
    writeOutcome("infra_failure");
  }
  return 0;
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

function initialPrompt(context, reviewer) {
  return `
你是 Bong PR review 面板中的 Reviewer ${reviewer.id}：${reviewer.name}。
你的重点：${reviewer.focus}

${GUIDELINES}

请审查 PR #${context.pr}。你可以用只读工具打开仓库真实文件、grep 调用方、核对 plan 和测试。
不要修改文件。不要给泛泛建议；只报可核验问题。

${contextBlock(context)}

只输出 JSON，不要 markdown，不要前言：
{
  "reviewer": "${reviewer.id}",
  "vote": "APPROVE|REQUEST_CHANGES",
  "confidence": 0-100,
  "plan_intent": {
    "status": "aligned|misaligned|not_plan|unclear",
    "reason": "是否符合 plan 原意的确认说明",
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
}
`.trim();
}

function finalPrompt(context, reviewer, peerResults) {
  return `
你是 Reviewer ${reviewer.id}：${reviewer.name}。你已经完成首轮审查，现在进入 4 人博弈复投。
请阅读其他 reviewer 的首轮意见，独立判断是否需要调整你的结论。不要为了制造共识而妥协。

通过标准：
- PR 若关联 plan，必须真正符合 plan 原意和交付物。
- 不存在 blocker/major 的正确性、断链、守恒、schema、测试或代码质量问题。
- 注释应简洁易懂，代码应直接可维护。
- 不确定时投 REQUEST_CHANGES。

${GUIDELINES}

## 首轮意见
${JSON.stringify(peerResults, null, 2)}

${contextBlock(context, { includeDiff: false })}

只输出 JSON，不要 markdown，不要前言：
{
  "reviewer": "${reviewer.id}",
  "vote": "APPROVE|REQUEST_CHANGES",
  "confidence": 0-100,
  "plan_intent": {
    "status": "aligned|misaligned|not_plan|unclear",
    "reason": "最终确认：是否符合 plan 原意",
    "missing": ["仍缺什么"]
  },
  "summary": "最终结论",
  "findings": [
    {
      "severity": "blocker|major|minor",
      "file": "路径",
      "line": "行号或范围",
      "title": "一句话问题",
      "evidence": "关键证据",
      "recommendation": "修复建议"
    }
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
    : "## Diff\n复投阶段不重复粘贴完整 diff；请结合首轮意见、文件列表和只读工具核对真实仓库。";

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

function compactResultForPrompt(result) {
  return {
    reviewer: result.reviewer,
    name: result.name,
    vote: result.vote,
    confidence: result.confidence,
    plan_intent: result.plan_intent,
    summary: result.summary,
    findings: result.findings.map((f) => ({
      severity: f.severity,
      file: f.file,
      line: f.line,
      title: f.title,
      evidence: f.evidence,
    })),
  };
}

async function runCodex(prompt, label) {
  const tmp = mkdtempSync(join(tmpdir(), `bong-review-${label}-`));
  const outputFile = join(tmp, "last-message.md");
  const args = [
    "exec",
    "-m",
    MODEL,
    "-C",
    process.cwd(),
    "-s",
    "read-only",
    "--ephemeral",
    "--output-last-message",
    outputFile,
    "-c",
    'model_reasoning_effort="high"',
    "-",
  ];

  console.error(`▶ codex ${label}`);
  try {
    for (let attempt = 1; attempt <= CODEX_RETRIES; attempt += 1) {
      const remainingMs = reviewDeadlineMs - Date.now();
      const attemptTimeoutMs = boundedAttemptTimeout(CODEX_TIMEOUT_MS, remainingMs);
      if (attemptTimeoutMs <= 0) {
        return codexExecutionFailureJson(label, "Review reviewer 总预算已耗尽，停止启动新的 Codex 进程");
      }

      rmSync(outputFile, { force: true });
      const result = await spawnCodex(args, prompt, attemptTimeoutMs);
      const text = existsSync(outputFile) ? readFileSync(outputFile, "utf8") : result.stdout;
      if (text.trim()) {
        if (result.code !== 0) {
          console.error(`  codex ${label} exit=${result.code} signal=${result.signal || "-"}，但已产出 final message，继续解析`);
        }
        return text;
      }

      const failure = codexFailureText(result);
      if (attempt < CODEX_RETRIES && isRetryableCodexFailure(result)) {
        const waitMs = CODEX_RETRY_MS * attempt;
        if (reviewDeadlineMs - Date.now() > waitMs + 15_000) {
          console.error(`  codex ${label} 暂时失败，${waitMs}ms 后重试（${attempt + 1}/${CODEX_RETRIES}）: ${failure}`);
          await delay(waitMs);
          continue;
        }
      }

      console.error(`  codex ${label} failed: ${failure}`);
      return codexExecutionFailureJson(label, failure);
    }
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function codexExecutionFailureJson(label, failure) {
  return JSON.stringify({
    vote: "REQUEST_CHANGES",
    confidence: 0,
    plan_intent: { status: "unclear", reason: `Codex ${label} 执行失败`, missing: [] },
    summary: `Codex ${label} 执行失败：${failure}`,
    findings: [
      {
        severity: "major",
        file: ".github/scripts/review.mjs",
        line: "0",
        title: `Codex reviewer ${label} 执行失败`,
        evidence: failure,
        recommendation: "检查 Codex CLI、模型端点和 API key 后重新触发 /review。",
      },
    ],
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function spawnCodex(args, stdin, timeoutMs) {
  return new Promise((resolve) => {
    const env = {
      ...process.env,
      OPENAI_API_KEY: process.env.REVIEW_CODEX_API_KEY || process.env.OPENAI_API_KEY || "",
      CODEX_API_KEY: process.env.REVIEW_CODEX_API_KEY || process.env.CODEX_API_KEY || process.env.OPENAI_API_KEY || "",
    };
    const child = spawn("codex", args, {
      env,
      stdio: ["pipe", "pipe", "pipe"],
      detached: process.platform !== "win32",
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let settled = false;
    let killTimer = null;
    let forceResolveTimer = null;
    let timer = null;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      clearTimeout(killTimer);
      clearTimeout(forceResolveTimer);
      resolve(result);
    };
    timer = setTimeout(() => {
      timedOut = true;
      killCodexProcess(child, "SIGTERM");
      killTimer = setTimeout(() => killCodexProcess(child, "SIGKILL"), 5_000);
      forceResolveTimer = setTimeout(
        () => finish({ code: 124, signal: "SIGKILL", stdout, stderr: `${stderr}\nCodex 进程组超时后未正常关闭` }),
        10_000,
      );
    }, timeoutMs);

    child.stdout.on("data", (chunk) => {
      stdout = appendCap(stdout, chunk);
    });
    child.stderr.on("data", (chunk) => {
      stderr = appendCap(stderr, chunk);
    });
    child.on("error", (error) => {
      finish({ code: 127, signal: null, stdout, stderr: `${stderr}\n${error.message}` });
    });
    child.on("close", (code, signal) => {
      finish({ code: timedOut ? 124 : code, signal, stdout, stderr });
    });
    child.stdin.on("error", () => {});
    child.stdin.end(stdin);
  });
}

function killCodexProcess(child, signal) {
  try {
    if (process.platform !== "win32" && child.pid) process.kill(-child.pid, signal);
    else child.kill(signal);
  } catch {
    try {
      child.kill(signal);
    } catch {
      // 进程已退出。
    }
  }
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

export function codexFailureText(result) {
  const stderr = excerptLog(redactCodexPromptEcho(result.stderr || ""), 2000);
  const stdout = excerptLog(redactCodexPromptEcho(result.stdout || ""), 1200);
  return [
    `exit=${result.code}`,
    result.signal ? `signal=${result.signal}` : "",
    stderr ? `stderr: ${stderr}` : "",
    stdout ? `stdout: ${stdout}` : "",
  ]
    .filter(Boolean)
    .join(" | ");
}

export function isRetryableCodexFailure(result) {
  if (result?.code === 124) return true;
  const log = `${result?.stderr || ""}\n${result?.stdout || ""}`;
  return /(429|503|too many requests|service unavailable|temporarily unavailable|upstream_(?:error|400)|stream disconnected|connection (?:reset|closed)|timed? out|timeout)/i.test(
    log,
  );
}

export function redactCodexPromptEcho(value) {
  const text = String(value || "");
  const marker = "\n--------\nuser\n";
  const markerAt = text.indexOf(marker);
  if (markerAt < 0) return text;

  const promptStart = markerAt + marker.length;
  const rest = text.slice(promptStart);
  const diagnosticAt = rest.search(
    /\n(?:\d{4}-\d{2}-\d{2}T[^\n]*\b(?:ERROR|WARN)\b|ERROR:|error:|warning:|Turn failed|stream disconnected|OpenAI API error|status code:)/,
  );
  if (diagnosticAt < 0) {
    return `${text.slice(0, promptStart)}[prompt echo omitted]\n`;
  }
  return `${text.slice(0, promptStart)}[prompt echo omitted]\n${rest.slice(diagnosticAt + 1)}`;
}

export function excerptLog(value, limit) {
  const text = String(value || "").trim();
  if (text.length <= limit) return text;
  const head = Math.floor(limit * 0.45);
  const tail = Math.max(200, limit - head - 80);
  return `${text.slice(0, head)}\n...[truncated ${text.length - head - tail} chars]...\n${text.slice(text.length - tail)}`;
}

export function isCodexExecutionFailure(result) {
  return (
    result?.confidence === 0 &&
    result?.findings?.some((finding) => finding.file === ".github/scripts/review.mjs" && /Codex reviewer .*执行失败/.test(finding.title))
  );
}

function renderComment(context, firstRound, finalRound, gate) {
  const findings = mergeFindings(finalRound);
  const planRows = finalRound
    .map((r) => `| ${r.reviewer} | ${r.plan_intent.status} | ${escapeCell(r.plan_intent.reason || "-")} |`)
    .join("\n");
  const voteRows = finalRound
    .map((r) => `| ${r.reviewer} ${r.name} | ${r.vote} | ${r.confidence} | ${escapeCell(r.summary || "-")} |`)
    .join("\n");
  const findingRows = findings.length
    ? findings
        .map(
          (f) =>
            `| ${f.severity} | ${f.file}:${f.line || "?"} | ${escapeCell(f.title)} | ${f.reviewers.join(",")} | ${escapeCell(f.recommendation || "")} |`,
        )
        .join("\n")
    : "| - | - | 未发现高置信度阻塞问题 | - | - |";

  const firstRoundDetails = JSON.stringify(firstRound.map(compactResultForPrompt), null, 2);
  const finalRoundDetails = JSON.stringify(finalRound.map(compactResultForPrompt), null, 2);
  const passLine = gate.passed ? "✅ **通过**" : "❌ **未通过**";
  const tieNote = gate.status === "TIE" ? "\n\n> 4 人复投为 2/2 平票；按规则平票不能通过，需要修正或人工复核后重新 `/review`。" : "";

  const body = `
## 🔭 Review · PR #${context.pr}

${passLine}：${gate.label}${tieNote}

> 引擎：4 个 Codex reviewer，模型 \`${MODEL}\`，reasoning high，base_url 默认 \`https://api.hlool.top\`。
> 触发：PR 首次创建自动跑；后续提交不自动跑，需要评论 \`/review\` 复审。
${context.plan ? `> Plan：\`${context.plan.name}\`${context.plan.path ? ` (${context.plan.path})` : "（未找到文件）"}` : "> Plan：未检测到 plan"}
${context.diffTruncated ? `> Diff 已截断至 ${MAX_DIFF} 字符，reviewer 可继续用只读工具查仓库。` : ""}

**📋 Plan 原意确认**

| Reviewer | 状态 | 说明 |
|---|---|---|
${planRows}

**🧑‍⚖️ 复投结果**

| Reviewer | Vote | Confidence | Summary |
|---|---:|---:|---|
${voteRows}

**🔎 Findings**

| 严重度 | 位置 | 问题 | Reviewer | 建议 |
|---|---|---|---|---|
${findingRows}

<details>
<summary>首轮与复投原始结构化摘要</summary>

\`\`\`json
${truncate(firstRoundDetails, 24_000)}
\`\`\`

\`\`\`json
${truncate(finalRoundDetails, 24_000)}
\`\`\`
</details>
`.trim();

  return truncate(body, 64_000);
}

function requirePrNumber() {
  if (!PR || !/^\d+$/.test(String(PR))) {
    throw new Error("PR_NUMBER 未设置或非法，必须是纯数字。");
  }
}

function currentCircuitState(events, now = nowIso()) {
  return evaluateCircuit(events, now, {
    threshold: CIRCUIT_THRESHOLD,
    windowMs: CIRCUIT_WINDOW_MS,
    durationMs: CIRCUIT_DURATION_MS,
  });
}

function nowIso() {
  const configured = process.env.REVIEW_NOW;
  const date = configured ? new Date(configured) : new Date();
  if (!Number.isFinite(date.getTime())) throw new Error(`REVIEW_NOW 非法：${configured}`);
  return date.toISOString();
}

function loadCircuitEvents() {
  return parseTrustedCircuitEvents(ensureCircuitStateIssues().flatMap(listIssueComments));
}

function findCircuitStateIssues() {
  const repo = resolveRepository();
  const pages = JSON.parse(gh(["api", "--paginate", "--slurp", `repos/${repo}/issues?state=all&per_page=100`]));
  const issues = Array.isArray(pages[0]) ? pages.flat() : pages;
  return selectCircuitStateIssues(issues);
}

function ensureCircuitStateIssues() {
  const existing = findCircuitStateIssues();
  if (existing.length) return existing;

  const repo = resolveRepository();
  const created = JSON.parse(
    gh([
      "api",
      `repos/${repo}/issues`,
      "-f",
      `title=${CIRCUIT_STATE_TITLE}`,
      "-f",
      `body=此 issue 由 Review Action 自动维护，用隐藏 marker 持久化 reviewer 基础设施失败；请勿手工编辑。\n\n${CIRCUIT_STATE_MARKER}`,
    ]),
  );
  return resolveCircuitStateIssueNumbers(created.number, findCircuitStateIssues());
}
function listIssueComments(issue) {
  const repo = resolveRepository();
  const pages = JSON.parse(gh(["api", "--paginate", "--slurp", `repos/${repo}/issues/${issue}/comments?per_page=100`]));
  return Array.isArray(pages[0]) ? pages.flat() : pages;
}

async function recordInfrastructureFailure(phase, reason) {
  requirePrNumber();
  const event = {
    v: 1,
    kind: "infra_failure",
    at: nowIso(),
    pr: Number(PR),
    run_id: process.env.GITHUB_RUN_ID || "",
    phase: truncate(String(phase || "unknown"), 80),
    reason: truncate(String(reason || "reviewer 执行失败"), 1200),
  };
  const circuitEvent = normalizeCircuitEvent(event);

  let events = [];
  if (circuitEvent) {
    try {
      const issues = ensureCircuitStateIssues();
      events = parseTrustedCircuitEvents(issues.flatMap(listIssueComments));
      postIssueComment(issues[0], `Review infra failure · PR #${PR} · ${event.phase}\n\n${renderHiddenMarker(CIRCUIT_MARKER, circuitEvent)}`);
    } catch (error) {
      console.error(`::warning::持久化 Review infra failure 失败：${errorText(error)}`);
    }
  } else {
    console.error("::warning::GITHUB_RUN_ID 缺失或非法，本次仅发布 handoff，不写入熔断计数。");
  }

  const combinedEvents =
    circuitEvent && !events.some((existing) => existing.run_id === circuitEvent.run_id) ? [...events, circuitEvent] : events;
  const state = currentCircuitState(combinedEvents, event.at);
  const body = renderInfrastructureHandoffComment(event, state);
  try {
    if (DRY_RUN) console.log(body);
    else postIssueComment(PR, body);
  } catch (error) {
    console.error(`::warning::发布 Review 降级评论失败：${errorText(error)}`);
  }
}

export function renderInfrastructureHandoffComment(event, state) {
  const circuitLine = state.open
    ? `\n\n已达到基础设施失败阈值，自动触发将熔断至 **${state.openUntil}**；评论 \`/review\` 或 \`workflow_dispatch\` 可始终手动旁路重试。`
    : "";
  return `## ⚠️ Review Action 基础设施降级\n\n本次 Action review 结果请忽略，改走 agent 自己的博弈式 review 流程，并会向用户反馈。\n\n失败阶段：\`${event.phase}\`。${circuitLine}\n\n${renderHiddenMarker(HANDOFF_MARKER, event)}`;
}

export function renderCircuitSkipComment(state) {
  const marker = {
    v: 1,
    kind: "circuit_skip",
    at: nowIso(),
    pr: Number(PR),
    run_id: process.env.GITHUB_RUN_ID || "",
    open_until: state.openUntil,
  };
  return `## ⚡ Review Action 自动熔断跳过\n\nreviewer 基础设施熔断中，本次自动触发已快速跳过并成功退出，不影响其他 CI。\n\n熔断截止：**${state.openUntil}**。评论 \`/review\` 或使用 \`workflow_dispatch\` 可始终手动旁路重试。\n\n${renderHiddenMarker(HANDOFF_MARKER, marker)}`;
}

function postIssueComment(issue, body) {
  const repo = resolveRepository();
  gh(["api", `repos/${repo}/issues/${issue}/comments`, "-f", `body=${body}`]);
}

function resolveRepository() {
  if (process.env.GITHUB_REPOSITORY) return process.env.GITHUB_REPOSITORY;
  return gh(["repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner"]).trim();
}

function writeOutcome(kind, extra = {}) {
  writeFileSync(OUTCOME_FILE, `${JSON.stringify({ kind, ...extra })}\n`);
}

function readOutcome() {
  if (!existsSync(OUTCOME_FILE)) return null;
  try {
    return JSON.parse(readFileSync(OUTCOME_FILE, "utf8"));
  } catch {
    return null;
  }
}

function setOutput(name, value) {
  const output = process.env.GITHUB_OUTPUT;
  if (output) writeFileSync(output, `${name}=${value}\n`, { flag: "a" });
  else console.log(`${name}=${value}`);
}

function errorText(error) {
  return excerptLog(error?.stderr || error?.message || String(error), 1200);
}
function gh(args) {
  return execFileSync("gh", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024, timeout: GH_TIMEOUT_MS });
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

function normalizeSeverity(value) {
  const s = String(value || "").toLowerCase();
  return ["blocker", "major", "minor"].includes(s) ? s : "minor";
}

function severityRank(value) {
  return { blocker: 0, major: 1, minor: 2 }[normalizeSeverity(value)] ?? 3;
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

const isEntry = import.meta.url === `file://${process.argv[1]}`;
if (isEntry) {
  main()
    .then((code) => {
      process.exitCode = code || 0;
    })
    .catch((error) => {
      console.error(error);
      process.exitCode = 1;
    });
}
