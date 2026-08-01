#!/usr/bin/env node
// Review v3 —— 4 个 Codex gpt-5.6-sol high reviewer 对同一 PR 做博弈式审核。
//
// 流程：
// 1. 拉取 PR metadata / diff / 关联 plan。
// 2. 4 个 reviewer 独立审查，分别盯 plan 原意、运行接线、正确性、代码质量。
// 3. 把首轮意见互相公开，4 个 reviewer 复投最终票。
// 4. 只有 3/1 或 4/0 APPROVE 才通过；2/2 直接视为未通过。
//
// 依赖：Node 内置模块 + gh + OpenAI-compatible Responses 端点。无 npm runtime dependency。

import { execFileSync } from "node:child_process";
import { existsSync, lstatSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";

const PR = process.env.PR_NUMBER;
const MODEL = process.env.REVIEW_CODEX_MODEL || "gpt-5.6-sol";
const MAX_DIFF = intEnv("REVIEW_MAX_DIFF", 40_000, 10_000);
const MAX_PLAN = intEnv("REVIEW_MAX_PLAN", 20_000, 5_000);
const CODEX_TIMEOUT_MS = intEnv("REVIEW_CODEX_TIMEOUT_MS", 900_000, 120_000);
const REVIEW_TOTAL_TIMEOUT_MS = intEnv("REVIEW_TOTAL_TIMEOUT_MINUTES", 60, 5) * 60_000;
const REVIEWER_FLOOR_MS = intEnv("REVIEW_REVIEWER_FLOOR_MS", 120_000, 1_000);
const REVIEW_CLEANUP_RESERVE_MS = 180_000;
export const REVIEWER_BUDGET_STARVED = "REVIEWER_BUDGET_STARVED";
const GH_TIMEOUT_MS = intEnv("REVIEW_GH_TIMEOUT_MS", 30_000, 1_000);
const CODEX_CONCURRENCY = intEnv("REVIEW_CODEX_CONCURRENCY", 1, 1);
const CODEX_RETRIES = intEnv("REVIEW_CODEX_RETRIES", 3, 1);
const CODEX_RETRY_MS = intEnv("REVIEW_CODEX_RETRY_MS", 15_000, 1_000);
const RESPONSES_BASE_URL = process.env.REVIEW_CODEX_BASE_URL || "https://api.claudeopus.world";
const DRY_RUN = /^(1|true|yes)$/i.test(String(process.env.REVIEW_DRY_RUN || "").trim());
const FAIL_ON_GATE = process.env.REVIEW_FAIL_ON_GATE !== "0";
const CIRCUIT_MARKER = "bong-review-circuit";
const HANDOFF_MARKER = "bong-review-handoff";
const CIRCUIT_STATE_MARKER = "<!-- bong-review-circuit-state:v1 -->";
const CIRCUIT_STATE_TITLE = "[automation] Review infrastructure circuit state";
const CIRCUIT_SEARCH_ATTEMPTS = 4;
const CIRCUIT_SEARCH_INTERVAL_MS = intEnv("REVIEW_CIRCUIT_SEARCH_INTERVAL_MS", 15_000, 1);
const CIRCUIT_OPERATION_BUDGET_MS = 120_000;
const CIRCUIT_THRESHOLD = intEnv("REVIEW_INFRA_FAILURE_THRESHOLD", 3, 1);
const CIRCUIT_WINDOW_MS = intEnv("REVIEW_INFRA_FAILURE_WINDOW_MINUTES", 60, 1) * 60_000;
const CIRCUIT_DURATION_MS = intEnv("REVIEW_CIRCUIT_MINUTES", 60, 1) * 60_000;
const OUTCOME_FILE = process.env.REVIEW_OUTCOME_FILE || "/tmp/review-run-outcome.json";
const COMMENT_FILE = process.env.REVIEW_COMMENT_FILE || "/tmp/review.md";
const DEFER_COMMENT = /^(1|true|yes)$/i.test(String(process.env.REVIEW_DEFER_COMMENT || "").trim());
const DEFERRED_RESULT = /^(1|true|yes)$/i.test(String(process.env.REVIEW_DEFERRED_RESULT || "").trim());
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

export function isCircuitBypassTrigger(eventName, commentBody) {
  return eventName === "issue_comment" && commentBody === "/review";
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
    !isCanonicalUtcTimestamp(value.at)
  ) {
    return null;
  }
  return { ...value, run_id: runId };
}

function isCanonicalUtcTimestamp(value) {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(value)) return false;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) && new Date(timestamp).toISOString() === value;
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
  return [
    ...new Set(
      (issues || [])
        .filter(
          (issue) =>
            !issue?.pull_request &&
            issue?.user?.login === "github-actions[bot]" &&
            issue?.user?.type === "Bot" &&
            issue?.title === CIRCUIT_STATE_TITLE &&
            String(issue.body || "").includes(CIRCUIT_STATE_MARKER),
        )
        .map((issue) => String(issue.number))
        .filter((number) => /^\d+$/.test(number)),
    ),
  ].sort((a, b) => Number(a) - Number(b));
}

export function resolveCircuitStateIssueNumbers(createdNumber, refreshedNumbers) {
  const created = String(createdNumber);
  if (!/^\d+$/.test(created)) throw new Error(`新建 Review 熔断状态 issue number 非法：${createdNumber}`);
  const refreshed = (refreshedNumbers || []).map(String).filter((number) => /^\d+$/.test(number));
  return [...new Set([created, ...refreshed])].sort((a, b) => Number(a) - Number(b));
}

export function buildCircuitStateSearchQuery(repo, title = CIRCUIT_STATE_TITLE) {
  const normalizedRepo = String(repo || "").trim();
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(normalizedRepo)) {
    throw new Error(`GITHUB_REPOSITORY 非法：${repo}`);
  }
  if (title !== CIRCUIT_STATE_TITLE || /[\u0000-\u001f\u007f"\\]/u.test(title)) {
    throw new Error("Review 熔断状态 title 非法。");
  }
  return `repo:${normalizedRepo} is:issue in:title "${title}"`;
}

export function parseGitHubJsonLines(output) {
  const lines = String(output || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  return lines.map((line) => JSON.parse(line));
}

export function boundedAttemptTimeout(configuredMs, remainingMs, cleanupReserveMs = REVIEW_CLEANUP_RESERVE_MS) {
  return Math.max(0, Math.min(configuredMs, remainingMs - cleanupReserveMs));
}

export function reviewerAttemptBudget({
  configuredMs,
  remainingMs,
  queuedSlots,
  floorMs = REVIEWER_FLOOR_MS,
  cleanupReserveMs = REVIEW_CLEANUP_RESERVE_MS,
}) {
  if (![configuredMs, remainingMs, queuedSlots, floorMs, cleanupReserveMs].every(Number.isFinite)) {
    throw new Error("Review reviewer 预算参数非法。");
  }
  if (!Number.isInteger(queuedSlots) || queuedSlots < 1 || floorMs <= 0 || cleanupReserveMs < 0) {
    throw new Error("Review reviewer 预算参数非法。");
  }
  const futureReserveMs = (queuedSlots - 1) * floorMs;
  const timeoutMs = boundedAttemptTimeout(configuredMs, remainingMs - futureReserveMs, cleanupReserveMs);
  return {
    timeoutMs,
    futureReserveMs,
    starvation: timeoutMs < floorMs,
  };
}

export function reviewerSlotsForRound(
  round,
  reviewerIndex,
  reviewerCount = REVIEWERS.length,
  activeSlots = 0,
) {
  if (
    ![reviewerIndex, reviewerCount, activeSlots].every(Number.isInteger) ||
    reviewerIndex < 0 ||
    reviewerIndex >= reviewerCount ||
    activeSlots < 0
  ) {
    throw new Error("Review reviewer 槽位参数非法。");
  }
  const remainingRoundSlots = reviewerCount - reviewerIndex;
  if (round === "initial") {
    return activeSlots + remainingRoundSlots + reviewerCount;
  }
  if (round === "final") return activeSlots + remainingRoundSlots;
  throw new Error(`Review reviewer round 非法：${round}`);
}

export function reviewerRetryBudget({
  remainingMs,
  queuedSlots,
  retryDelayMs,
  floorMs = REVIEWER_FLOOR_MS,
  cleanupReserveMs = REVIEW_CLEANUP_RESERVE_MS,
}) {
  if (![remainingMs, queuedSlots, retryDelayMs, floorMs, cleanupReserveMs].every(Number.isFinite)) {
    throw new Error("Review reviewer 重试预算参数非法。");
  }
  const afterWaitMs = remainingMs - retryDelayMs;
  const budget = reviewerAttemptBudget({
    configuredMs: floorMs,
    remainingMs: afterWaitMs,
    queuedSlots,
    floorMs,
    cleanupReserveMs,
  });
  return {
    ...budget,
    afterWaitMs,
    retryAllowed: budget.timeoutMs >= floorMs,
  };
}

export function isReviewerBudgetStarvation(result) {
  return result?.reviewer_budget_starvation === true;
}

export function reviewerBudgetStarvationResult(label, budget) {
  return {
    raw: "",
    executionFailure: false,
    reviewerBudgetStarvation: true,
    summary: `${REVIEWER_BUDGET_STARVED}: ${label} 未发起 Responses 请求；剩余 ${budget.remainingMs}ms 无法同时保留 ${budget.queuedSlots} 个 reviewer 的最低预算。`,
  };
}

export function circuitOperationDeadlines(
  startMs,
  stateBudgetMs = CIRCUIT_OPERATION_BUDGET_MS,
  commentBudgetMs = GH_TIMEOUT_MS,
) {
  if (![startMs, stateBudgetMs, commentBudgetMs].every(Number.isFinite) || stateBudgetMs <= 0 || commentBudgetMs <= 0) {
    throw new Error("Review 熔断操作预算非法。");
  }
  return {
    stateDeadlineMs: startMs + stateBudgetMs,
    commentDeadlineMs: startMs + stateBudgetMs + commentBudgetMs,
  };
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
  const firstResults = firstRound || [];
  const finalResults = finalRound || [];
  const allResults = [...firstResults, ...finalResults];
  if (collectRealFindings(allResults).length) return "gate_failure";
  if (
    firstResults.length !== REVIEWERS.length ||
    finalResults.length !== REVIEWERS.length ||
    allResults.some(isReviewInfrastructureFailure)
  ) {
    return "infra_failure";
  }
  return decideGate(finalResults).passed ? "passed" : "gate_failure";
}

function isReviewInfrastructureFailure(result) {
  return result?.execution_failure === true || isReviewerBudgetStarvation(result);
}

function isPureReviewInfrastructureFailure(result) {
  return isCodexExecutionFailure(result) || isReviewerBudgetStarvation(result);
}

export function decideReviewGate(results, findingResults = results) {
  const voteGate = decideGate(results);
  const realFindings = collectRealFindings(findingResults);
  if (!realFindings.length) return voteGate;
  return {
    ...voteGate,
    passed: false,
    status: "REQUEST_CHANGES",
    label: `${voteGate.approve}/${voteGate.request}，存在 ${realFindings.length} 项真实 finding，要求修改`,
  };
}

function collectRealFindings(results) {
  const unique = new Map();
  for (const result of results || []) {
    for (const finding of result.findings || []) {
      if (result.execution_failure === true && isCodexFailureFinding(finding)) continue;
      const key = `${finding.file || ""}|${finding.line || ""}|${String(finding.title || "").trim().toLowerCase()}`;
      if (!unique.has(key)) unique.set(key, finding);
    }
  }
  return [...unique.values()];
}

export function classifyWorkflowFinalization({
  circuit = "success",
  reviewJob = "success",
  install = "skipped",
  configure = "skipped",
  checkout = "success",
  review = "skipped",
  outcomeKind = null,
}) {
  if (circuit !== "success") {
    return {
      kind: "infra_failure",
      phase: "circuit_preflight",
      reason: `Review 熔断预检未成功（${circuit}）`,
      shouldRecord: true,
    };
  }
  if (reviewJob !== "success") {
    return {
      kind: "infra_failure",
      phase: "review_job",
      reason: `只读 Review job 未成功（${reviewJob}）`,
      shouldRecord: true,
    };
  }
  if (install !== "success") {
    return {
      kind: "infra_failure",
      phase: "cli_install",
      reason: `Codex CLI 安装或版本检查未成功（${install}）`,
      shouldRecord: true,
    };
  }
  if (configure !== "success") {
    return {
      kind: "infra_failure",
      phase: "provider_config",
      reason: `Codex provider 配置未成功（${configure}）`,
      shouldRecord: true,
    };
  }
  if (checkout !== "success") {
    return {
      kind: "infra_failure",
      phase: "pr_checkout",
      reason: `PR head checkout 或 commit 校验未成功（${checkout}）`,
      shouldRecord: true,
    };
  }
  if (outcomeKind === "gate_failure") {
    return { kind: "gate_failure", shouldRecord: false };
  }
  if (outcomeKind === "infra_failure") {
    return { kind: "infra_failure", shouldRecord: false };
  }
  if (review !== "success" || outcomeKind !== "passed") {
    return {
      kind: "infra_failure",
      phase: "review_process",
      reason:
        review === "failure"
          ? "Review 脚本、CLI 或沙箱异常退出，且未产出可识别结果"
          : `Review 步骤或结果异常（step=${review}, outcome=${outcomeKind || "missing"}）`,
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

export function normalizeResult(raw, reviewer, { executionFailure = false } = {}) {
  const parsed = extractJSON(raw);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return {
      execution_failure: executionFailure,
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
    execution_failure: executionFailure,
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
      if (!current.reviewers.includes(result.reviewer)) current.reviewers.push(result.reviewer);
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
    return finishInfrastructureFailure("provider_config", "缺 REVIEW_CODEX_API_KEY / OPENAI_API_KEY");
  }

  const context = loadPrContext(PR);
  console.error(`Review v3: PR #${PR} · ${context.changedLines} 行/${context.changedFiles} 文件 · 4×${MODEL} high`);
  const { firstRound, finalRound, outcome } = await runReviewPanel(context);
  if (outcome === "infra_failure") {
    const failed = [...new Map(
      [...firstRound, ...finalRound]
        .filter(isReviewInfrastructureFailure)
        .map((result) => [`${result.reviewer}|${result.summary}`, result]),
    ).values()];
    const reason = failed.map((result) => result.summary).filter(Boolean).join(" | ");
    return finishInfrastructureFailure("reviewer_execution", reason || "Codex reviewer 执行失败");
  }

  const gate = decideReviewGate(finalRound, [...firstRound, ...finalRound]);
  const body = renderComment(context, firstRound, finalRound, gate);
  writeFileSync(COMMENT_FILE, body);

  if (DRY_RUN) {
    console.log(body);
  } else if (!DEFER_COMMENT) {
    gh(["pr", "comment", PR, "--body-file", COMMENT_FILE]);
    console.error("已发布 review 评论。");
  } else {
    console.error("review 评论已延迟到可信 finalize job 发布。");
  }

  writeOutcome(gate.passed ? "passed" : "gate_failure", { gate: gate.status });
  if (FAIL_ON_GATE && !gate.passed) {
    console.error(`Review gate 未通过：${gate.label}`);
    return 1;
  }
  return 0;
}

export async function runReviewPanel(
  context,
  reviewerRunner = runCodex,
  { concurrency = CODEX_CONCURRENCY } = {},
) {
  const runRound = (round, promptForReviewer) =>
    mapLimit(REVIEWERS, concurrency, (reviewer, reviewerIndex, activeSlots) =>
      reviewerRunner(promptForReviewer(reviewer), `${round}-${reviewer.id}`, {
        round,
        reviewerIndex,
        queuedSlots: reviewerSlotsForRound(
          round,
          reviewerIndex,
          REVIEWERS.length,
          activeSlots,
        ),
      }).then((result) => normalizeReviewerResult(result, reviewer)),
    );

  const firstRound = await runRound("initial", (reviewer) => initialPrompt(context, reviewer));
  const debateContext = firstRound.map(compactResultForPrompt);
  const finalRoundRaw = firstRound.every(isPureReviewInfrastructureFailure)
    ? firstRound
    : await runRound("final", (reviewer) => finalPrompt(context, reviewer, debateContext));
  const finalRound = applyPlanIntentGate(finalRoundRaw, Boolean(context.plan));
  return { firstRound, finalRound, outcome: classifyReviewRun(firstRound, finalRound) };
}

function normalizeReviewerResult(result, reviewer) {
  if (result.reviewerBudgetStarvation) {
    return {
      execution_failure: false,
      reviewer_budget_starvation: true,
      reviewer: reviewer.id,
      name: reviewer.name,
      vote: "REQUEST_CHANGES",
      confidence: 0,
      plan_intent: { status: "unclear", reason: REVIEWER_BUDGET_STARVED, missing: [] },
      summary: result.summary,
      findings: [],
    };
  }
  return normalizeResult(result.raw, reviewer, { executionFailure: result.executionFailure });
}

async function finishInfrastructureFailure(phase, reason) {
  const safeReason = truncate(String(reason || "Review 基础设施失败"), 4000);
  if (!DEFER_COMMENT) await recordInfrastructureFailure(phase, safeReason);
  writeOutcome("infra_failure", { phase, reason: safeReason });
  return 0;
}

async function circuitPreflight() {
  requirePrNumber();
  const trigger = process.env.REVIEW_TRIGGER || process.env.GITHUB_EVENT_NAME || "";
  const commentBody = process.env.REVIEW_COMMENT_BODY || "";
  if (isCircuitBypassTrigger(trigger, commentBody)) {
    console.error(`手动触发 ${trigger} 旁路 Review 熔断。`);
    setOutput("should_run", "true");
    return 0;
  }

  const deadlines = circuitOperationDeadlines(Date.now());
  let state;
  try {
    state = currentCircuitState(loadCircuitEvents(deadlines.stateDeadlineMs));
  } catch (error) {
    console.error(`::warning::读取 Review 熔断状态失败，按 fail-open 继续执行：${errorText(error)}`);
  }
  if (state?.open) {
    const body = renderCircuitSkipComment(state);
    try {
      if (DRY_RUN) console.log(body);
      else postIssueComment(PR, body, deadlines.commentDeadlineMs);
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
  const outcome = readOutcome();
  const decision = classifyWorkflowFinalization({
    circuit: process.env.REVIEW_CIRCUIT_OUTCOME || "success",
    reviewJob: process.env.REVIEW_JOB_OUTCOME || "success",
    install: process.env.REVIEW_INSTALL_OUTCOME || "skipped",
    configure: process.env.REVIEW_CONFIGURE_OUTCOME || "skipped",
    checkout: process.env.REVIEW_CHECKOUT_OUTCOME || "success",
    review: process.env.REVIEW_STEP_OUTCOME || "skipped",
    outcomeKind: outcome?.kind || null,
  });

  if (decision.kind === "infra_failure") {
    if (decision.shouldRecord || DEFERRED_RESULT) {
      await recordInfrastructureFailure(
        decision.phase || outcome?.phase || "review_process",
        decision.reason || outcome?.reason || "Review 基础设施失败",
      );
      writeOutcome("infra_failure", {
        phase: decision.phase || outcome?.phase || "review_process",
      });
    }
    return 0;
  }

  if (DEFERRED_RESULT) {
    try {
      if (!existsSync(COMMENT_FILE)) throw new Error(`缺延迟 review 评论文件：${COMMENT_FILE}`);
      gh(["pr", "comment", PR, "--body-file", COMMENT_FILE]);
      console.error("可信 finalize job 已发布 review 评论。");
    } catch (error) {
      await recordInfrastructureFailure("review_comment", errorText(error));
      writeOutcome("infra_failure", { phase: "review_comment" });
      return 0;
    }
  }

  return decision.kind === "gate_failure" ? 1 : 0;
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
    const text = readWorkspaceRegularFile(path);
    if (text !== null) return { name, path, text: truncate(text, MAX_PLAN) };
  }
  return { name, path: null, text: "" };
}

export function readWorkspaceRegularFile(path, workspace = process.cwd()) {
  try {
    const workspaceReal = realpathSync(workspace);
    const candidate = resolve(workspaceReal, path);
    const candidateRelative = relative(workspaceReal, candidate);
    if (candidateRelative === ".." || candidateRelative.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`)) return null;
    if (!lstatSync(candidate).isFile()) return null;
    const candidateReal = realpathSync(candidate);
    const realRelative = relative(workspaceReal, candidateReal);
    if (realRelative === ".." || realRelative.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`)) return null;
    return readFileSync(candidateReal, "utf8");
  } catch {
    return null;
  }
}

function initialPrompt(context, reviewer) {
  return `
你是 Bong PR review 面板中的 Reviewer ${reviewer.id}：${reviewer.name}。
你的重点：${reviewer.focus}

${GUIDELINES}

请审查 PR #${context.pr}。请求不提供任何工具；下面已包含 PR diff、文件清单和关联 plan。
只根据这些材料审查，不要给泛泛建议；只报可核验问题。

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

${contextBlock(context, { includeDiff: true })}

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

async function runCodex(prompt, label, scheduling = {}) {
  return runCodexResponses(prompt, label, scheduling);
}

export async function runCodexResponses(
  prompt,
  label,
  {
    queuedSlots = 1,
    now = Date.now,
    request = requestResponses,
    wait = delay,
    deadlineMs = reviewDeadlineMs,
    configuredMs = CODEX_TIMEOUT_MS,
    floorMs = REVIEWER_FLOOR_MS,
    cleanupReserveMs = REVIEW_CLEANUP_RESERVE_MS,
    retries = CODEX_RETRIES,
    retryMs = CODEX_RETRY_MS,
  } = {},
) {
  console.error(`▶ responses ${label}`);
  let requestStarted = false;
  for (let attempt = 1; attempt <= retries; attempt += 1) {
    const remainingMs = deadlineMs - now();
    const budget = reviewerAttemptBudget({
      configuredMs,
      remainingMs,
      queuedSlots,
      floorMs,
      cleanupReserveMs,
    });
    if (budget.starvation) {
      if (!requestStarted) return reviewerBudgetStarvationResult(label, { ...budget, remainingMs, queuedSlots });
      return {
        raw: codexExecutionFailureJson(label, "Review reviewer 重试预算不足，停止新的 Responses 请求"),
        executionFailure: true,
      };
    }

    requestStarted = true;
    const result = await request(prompt, budget.timeoutMs);
    const text = redactRuntimeSecrets(result.stdout || "");
    const zeroConfidenceInfra = isZeroConfidenceWithoutCodeFindings(text);
    if (isSuccessfulCodexResponse(result, text)) return { raw: text, executionFailure: false };

    const failure = codexFailureText(result);
    const retryableFailure = zeroConfidenceInfra || isRetryableCodexFailure(result);
    if (attempt < retries && retryableFailure) {
      const waitMs = codexRetryDelayMs(result, attempt, retryMs);
      const retryBudget = reviewerRetryBudget({
        remainingMs: deadlineMs - now(),
        queuedSlots,
        retryDelayMs: waitMs,
        floorMs,
        cleanupReserveMs,
      });
      if (retryBudget.retryAllowed) {
        console.error(`  responses ${label} 暂时失败，${waitMs}ms 后重试（${attempt + 1}/${retries}）: ${failure}`);
        await wait(waitMs);
        continue;
      }
    }

    console.error(`  responses ${label} failed: ${failure}`);
    const parsed = extractJSON(text);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return { raw: text, executionFailure: true };
    }
    return { raw: codexExecutionFailureJson(label, failure), executionFailure: true };
  }
}

export function isSuccessfulCodexResponse(result, text = result?.stdout || "") {
  return result?.code === 0 && String(text).trim().length > 0 && !isZeroConfidenceWithoutCodeFindings(text);
}

export function buildResponsesEndpoint(baseUrl, allowHttp = false) {
  const url = new URL(String(baseUrl || "").trim());
  if (url.username || url.password || url.search || url.hash) throw new Error("Review Responses base URL 不得含凭据、查询或 fragment。");
  if (url.protocol !== "https:" && !(allowHttp && url.protocol === "http:")) {
    throw new Error("Review Responses base URL 必须使用 HTTPS。");
  }
  const path = url.pathname.replace(/\/+$/, "");
  url.pathname = path.endsWith("/responses") ? path : path.endsWith("/v1") ? `${path}/responses` : `${path}/v1/responses`;
  return url.toString();
}

export function extractResponsesOutputText(payload) {
  if (typeof payload?.output_text === "string" && payload.output_text.trim()) return payload.output_text;
  const chunks = [];
  for (const item of payload?.output || []) {
    if (item?.type !== "message") continue;
    for (const content of item.content || []) {
      if ((content?.type === "output_text" || content?.type === "text") && typeof content.text === "string") {
        chunks.push(content.text);
      }
    }
  }
  return chunks.join("\n");
}

const MAX_RESPONSES_SSE_EVENT_BYTES = 4 * 1024 * 1024;
const MAX_RESPONSES_SSE_TOTAL_DATA_BYTES = 16 * 1024 * 1024;
const MAX_RESPONSES_SSE_BUFFER_BYTES = MAX_RESPONSES_SSE_EVENT_BYTES * 2;
const responsesSseEncoder = new TextEncoder();

function responsesSseError(message) {
  return new Error(`Responses SSE ${message}`);
}

function responseErrorDetail(payload, fallback) {
  const error = payload?.error || payload?.response?.error;
  if (typeof error === "string" && error.trim()) return error.trim();
  if (error && typeof error === "object") {
    const detail = error.message || error.code || error.type;
    if (detail) return String(detail);
  }
  const incompleteReason = payload?.response?.incomplete_details?.reason || payload?.incomplete_details?.reason;
  return incompleteReason ? String(incompleteReason) : fallback;
}

function reduceResponsesSseEvent(state, event) {
  const data = String(event?.data ?? "");
  if (data === "[DONE]") return state;
  if (!data.trim()) return state;

  let payload;
  try {
    payload = JSON.parse(data);
  } catch {
    throw responsesSseError("事件 data 不是合法 JSON。");
  }
  if (!payload || typeof payload !== "object" || Array.isArray(payload) || typeof payload.type !== "string" || !payload.type) {
    throw responsesSseError("事件必须是带非空 type 的 JSON 对象。");
  }
  const eventName = String(event?.event || "");
  if (eventName && eventName !== "message" && eventName !== payload.type) {
    throw responsesSseError(`event 字段与 type 不一致：${eventName} != ${payload.type}`);
  }

  switch (payload.type) {
    case "response.output_text.delta":
      if (typeof payload.delta !== "string") throw responsesSseError("output_text.delta 缺少字符串 delta。");
      state.deltaText += payload.delta;
      return state;
    case "response.completed": {
      if (!payload.response || typeof payload.response !== "object" || Array.isArray(payload.response)) {
        throw responsesSseError("response.completed 缺少 response 信封。");
      }
      state.stdout = extractResponsesOutputText(payload.response) || state.deltaText;
      state.completed = true;
      return state;
    }
    case "response.failed":
      state.failure = responseErrorDetail(payload, "response.failed");
      state.terminal = true;
      return state;
    case "response.incomplete":
      state.failure = responseErrorDetail(payload, "response.incomplete");
      state.terminal = true;
      return state;
    case "error":
      state.failure = responseErrorDetail(payload, "Responses provider error");
      state.terminal = true;
      return state;
    default:
      return state;
  }
}

async function consumeResponsesSse(body, state = {}) {
  if (!body || typeof body.getReader !== "function") throw responsesSseError("响应缺少可读 body。");
  const reader = body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let buffer = "";
  let bufferBytes = 0;
  let scanOffset = 0;
  let eventName = "";
  let dataLines = [];
  let dataBytes = 0;
  let totalDataBytes = 0;
  let stopped = false;
  const streamState = state;
  streamState.deltaText ??= "";
  streamState.stdout ??= "";
  streamState.completed ??= false;
  streamState.terminal ??= false;
  streamState.failure ??= "";

  const resetEvent = () => {
    eventName = "";
    dataLines = [];
    dataBytes = 0;
  };
  const dispatch = () => {
    if (!eventName && dataLines.length === 0) return;
    const event = { event: eventName, data: dataLines.join("\n") };
    resetEvent();
    reduceResponsesSseEvent(streamState, event);
  };
  const processLine = (line) => {
    if (line === "") {
      dispatch();
      return;
    }
    if (line.startsWith(":")) return;
    const colon = line.indexOf(":");
    const field = colon < 0 ? line : line.slice(0, colon);
    let value = colon < 0 ? "" : line.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    if (field === "event") {
      eventName = value;
    } else if (field === "data") {
      const valueBytes = responsesSseEncoder.encode(value).byteLength + 1;
      dataBytes += valueBytes;
      totalDataBytes += valueBytes;
      if (dataBytes > MAX_RESPONSES_SSE_EVENT_BYTES) throw responsesSseError("单个事件超过大小上限。");
      if (totalDataBytes > MAX_RESPONSES_SSE_TOTAL_DATA_BYTES) throw responsesSseError("累计事件数据超过大小上限。");
      dataLines.push(value);
    }
  };
  const processText = (text) => {
    buffer += text;
    bufferBytes += responsesSseEncoder.encode(text).byteLength;
    while (true) {
      let end = -1;
      let separatorLength = 0;
      for (let index = scanOffset; index < buffer.length; index += 1) {
        const char = buffer[index];
        if (char === "\n") {
          end = index;
          separatorLength = 1;
          break;
        }
        if (char === "\r") {
          if (index + 1 === buffer.length) {
            scanOffset = index;
            break;
          }
          end = index;
          separatorLength = buffer[index + 1] === "\n" ? 2 : 1;
          break;
        }
      }
      if (end < 0) {
        if (bufferBytes > MAX_RESPONSES_SSE_BUFFER_BYTES) {
          throw responsesSseError("未完成 frame 超过大小上限。");
        }
        return;
      }
      const line = buffer.slice(0, end);
      const consumed = buffer.slice(0, end + separatorLength);
      buffer = buffer.slice(end + separatorLength);
      bufferBytes -= responsesSseEncoder.encode(consumed).byteLength;
      scanOffset = 0;
      processLine(line);
      if (streamState.completed || streamState.terminal) return;
    }
  };

  try {
    while (!streamState.completed && !streamState.terminal) {
      const { done, value } = await reader.read();
      if (done) break;
      if (!(value instanceof Uint8Array)) throw responsesSseError("收到非字节 chunk。");
      processText(decoder.decode(value, { stream: true }));
    }
    if (!streamState.completed && !streamState.terminal) {
      processText(decoder.decode());
      if (!streamState.completed && !streamState.terminal) {
        if (buffer.length > 0) {
          if (buffer.endsWith("\r")) {
            processLine(buffer.slice(0, -1));
          } else {
            processLine(buffer);
          }
        }
        if (!streamState.completed && !streamState.terminal) {
          throw responsesSseError("stream disconnected before completion");
        }
      }
    }
    return streamState;
  } finally {
    if (!streamState.completed && !streamState.terminal) stopped = true;
    if (stopped || streamState.completed || streamState.terminal) {
      try {
        await reader.cancel();
      } catch {
        /* The provider may already have closed the stream. */
      }
    }
    reader.releaseLock();
  }
}

export async function requestResponses(prompt, timeoutMs, options = {}) {
  const fetchImpl = options.fetchImpl || globalThis.fetch;
  const apiKey = options.apiKey ?? process.env.REVIEW_CODEX_API_KEY ?? process.env.OPENAI_API_KEY ?? "";
  const endpoint = buildResponsesEndpoint(
    options.baseUrl || RESPONSES_BASE_URL,
    options.allowHttp === true,
  );
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const streamState = {
    deltaText: "",
    stdout: "",
    completed: false,
    terminal: false,
    failure: "",
  };
  try {
    const response = await fetchImpl(endpoint, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
        Accept: "text/event-stream",
      },
      body: JSON.stringify({
        model: options.model || MODEL,
        input: prompt,
        reasoning: { effort: "high" },
        store: false,
        stream: true,
      }),
      signal: controller.signal,
    });
    if (!response.ok) {
      const raw = await response.text();
      let payload = null;
      try {
        payload = raw ? JSON.parse(raw) : null;
      } catch {
        /* HTTP error text or malformed provider response is reported below. */
      }
      const output = extractResponsesOutputText(payload);
      const detail = payload?.error?.message || raw || response.statusText || "unknown provider error";
      return { code: response.status, signal: null, stdout: output, stderr: `HTTP ${response.status}: ${detail}` };
    }

    const state = await consumeResponsesSse(response.body, streamState);
    const output = state.stdout || state.deltaText || "";
    if (state.failure) return { code: 1, signal: null, stdout: output, stderr: state.failure };
    if (!state.completed) return { code: 1, signal: null, stdout: output, stderr: "Responses SSE stream disconnected before completion" };
    return {
      code: 0,
      signal: null,
      stdout: output,
      stderr: output ? "" : "Responses 未返回 output_text。",
    };
  } catch (error) {
    const timedOut = error?.name === "AbortError" || controller.signal.aborted;
    return {
      code: timedOut ? 124 : 1,
      signal: timedOut ? "SIGTERM" : null,
      stdout: streamState.stdout || streamState.deltaText || "",
      stderr: error?.message || String(error),
    };
  } finally {
    clearTimeout(timer);
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

async function mapLimit(items, limit, fn) {
  const out = new Array(items.length);
  let next = 0;
  let active = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) {
      const idx = next++;
      active += 1;
      try {
        out[idx] = await fn(items[idx], idx, active - 1);
      } finally {
        active -= 1;
      }
    }
  });
  await Promise.all(workers);
  return out;
}

export function codexFailureText(result) {
  const stderr = excerptLog(redactRuntimeSecrets(result.stderr || ""), 2000);
  const stdout = excerptLog(redactRuntimeSecrets(result.stdout || ""), 1200);
  return [
    `exit=${result.code}`,
    result.signal ? `signal=${result.signal}` : "",
    stderr ? `stderr: ${stderr}` : "",
    stdout ? `stdout: ${stdout}` : "",
  ]
    .filter(Boolean)
    .join(" | ");
}

export function isZeroConfidenceWithoutCodeFindings(raw) {
  const parsed = extractJSON(raw);
  return (
    parsed &&
    typeof parsed === "object" &&
    !Array.isArray(parsed) &&
    Number(parsed.confidence) === 0 &&
    Array.isArray(parsed.findings) &&
    parsed.findings.length === 0
  );
}

export function redactRuntimeSecrets(value, env = process.env) {
  let text = String(value || "");
  const secrets = Object.entries(env)
    .filter(([name, secret]) => /(KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|PROXY)/i.test(name) && String(secret || "").length >= 4)
    .map(([, secret]) => String(secret))
    .sort((a, b) => b.length - a.length);
  for (const secret of new Set(secrets)) text = text.replaceAll(secret, "[REDACTED]");
  return text;
}

export function codexRetryDelayMs(result, attempt, configuredMs = CODEX_RETRY_MS) {
  const baseDelay = Math.max(1_000, configuredMs * attempt);
  return result?.code === 429 ? Math.max(120_000, baseDelay) : baseDelay;
}

export function isRetryableCodexFailure(result) {
  if ([429, 503, 524].includes(Number(result?.code)) || result?.code === 124) return true;
  const log = `${result?.stderr || ""}\n${result?.stdout || ""}`;
  return /(429|503|too many requests|service unavailable|temporarily unavailable|upstream_(?:error|400)|stream disconnected|connection (?:reset|closed)|timed? out|timeout)/i.test(
    log,
  );
}

export function excerptLog(value, limit) {
  const text = String(value || "").trim();
  if (text.length <= limit) return text;
  const head = Math.floor(limit * 0.45);
  const tail = Math.max(200, limit - head - 80);
  return `${text.slice(0, head)}\n...[truncated ${text.length - head - tail} chars]...\n${text.slice(text.length - tail)}`;
}

export function isCodexExecutionFailure(result) {
  const findings = result?.findings;
  return (
    result?.execution_failure === true &&
    result?.confidence === 0 &&
    Array.isArray(findings) &&
    findings.every(isCodexFailureFinding)
  );
}

function isCodexFailureFinding(finding) {
  return finding?.file === ".github/scripts/review.mjs" && /Codex reviewer .*执行失败/.test(String(finding.title || ""));
}

export function renderComment(context, firstRound, finalRound, gate) {
  const findings = mergeFindings(reviewFindingResults(firstRound, finalRound));
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

> 引擎：4 个 Codex reviewer，模型 \`${MODEL}\`，reasoning high，base_url 默认 \`https://api.claudeopus.world\`。
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

export function reviewFindingResults(firstRound, finalRound) {
  return [...(firstRound || []), ...(finalRound || [])];
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

function loadCircuitEvents(deadlineMs = Date.now() + CIRCUIT_OPERATION_BUDGET_MS) {
  return parseTrustedCircuitEvents(ensureCircuitStateIssues({ deadlineMs }).flatMap((issue) => listIssueComments(issue, deadlineMs)));
}

export function findCircuitStateIssues(repo = resolveRepository(), runGh = gh, wait = sleepSync, timing = {}) {
  const now = timing.now || Date.now;
  const deadlineMs = timing.deadlineMs ?? now() + CIRCUIT_OPERATION_BUDGET_MS;
  const attempts = timing.searchAttempts ?? CIRCUIT_SEARCH_ATTEMPTS;
  const intervalMs = timing.searchIntervalMs ?? CIRCUIT_SEARCH_INTERVAL_MS;
  if (!Number.isInteger(attempts) || attempts < 1 || !Number.isFinite(intervalMs) || intervalMs < 1) {
    throw new Error("Review 熔断 Search 节流配置非法。");
  }
  const query = buildCircuitStateSearchQuery(repo);
  const args = [
    "api",
    "--paginate",
    "--method",
    "GET",
    "search/issues",
    "-f",
    `q=${query}`,
    "-f",
    "per_page=100",
    "--jq",
    ".items[]",
  ];
  const issueNumbers = new Set();
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const delayMs = Math.max(0, Number(timing.nextSearchAtMs || 0) - now());
    if (delayMs > 0) {
      if (deadlineMs - now() <= delayMs) throw new Error("Review 熔断状态查询预算已耗尽。");
      wait(delayMs);
    }
    const found = selectCircuitStateIssues(parseGitHubJsonLines(runGh(args, circuitGhTimeout(deadlineMs, now()))));
    found.forEach((number) => issueNumbers.add(number));
    timing.nextSearchAtMs = now() + intervalMs;
  }
  return [...issueNumbers].sort((a, b) => Number(a) - Number(b));
}

export function ensureCircuitStateIssues(options = {}) {
  const repo = options.repo || resolveRepository();
  const runGh = options.runGh || gh;
  const wait = options.wait || sleepSync;
  const now = options.now || Date.now;
  const deadlineMs = options.deadlineMs ?? now() + CIRCUIT_OPERATION_BUDGET_MS;
  const timing = {
    deadlineMs,
    now,
    searchAttempts: options.searchAttempts,
    searchIntervalMs: options.searchIntervalMs,
  };
  const existing = findCircuitStateIssues(repo, runGh, wait, timing);
  if (existing.length) return existing;

  const created = JSON.parse(
    runGh(
      [
        "api",
        `repos/${repo}/issues`,
        "-f",
        `title=${CIRCUIT_STATE_TITLE}`,
        "-f",
        `body=此 issue 由 Review Action 自动维护，用隐藏 marker 持久化 reviewer 基础设施失败；请勿手工编辑。\n\n${CIRCUIT_STATE_MARKER}`,
      ],
      circuitGhTimeout(deadlineMs, now()),
    ),
  );
  return resolveCircuitStateIssueNumbers(created.number, findCircuitStateIssues(repo, runGh, wait, timing));
}
function listIssueComments(issue, deadlineMs = Date.now() + CIRCUIT_OPERATION_BUDGET_MS) {
  const repo = resolveRepository();
  const output = gh(
    ["api", "--paginate", `repos/${repo}/issues/${issue}/comments?per_page=100`, "--jq", ".[]"],
    circuitGhTimeout(deadlineMs),
  );
  return parseGitHubJsonLines(output);
}

async function recordInfrastructureFailure(phase, reason) {
  requirePrNumber();
  const deadlines = circuitOperationDeadlines(Date.now());
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
  let eventPersisted = false;
  if (circuitEvent) {
    try {
      const issues = ensureCircuitStateIssues({ deadlineMs: deadlines.stateDeadlineMs });
      events = parseTrustedCircuitEvents(
        issues.flatMap((issue) => listIssueComments(issue, deadlines.stateDeadlineMs)),
      );
      eventPersisted = events.some((existing) => existing.run_id === circuitEvent.run_id);
      if (!eventPersisted) {
        postIssueComment(
          issues[0],
          `Review infra failure · PR #${PR} · ${event.phase}\n\n${renderHiddenMarker(CIRCUIT_MARKER, circuitEvent)}`,
          deadlines.stateDeadlineMs,
        );
        eventPersisted = true;
      }
    } catch (error) {
      console.error(`::warning::持久化 Review infra failure 失败：${errorText(error)}`);
    }
  } else {
    console.error("::warning::GITHUB_RUN_ID 缺失或非法，本次仅发布 handoff，不写入熔断计数。");
  }

  const combinedEvents =
    eventPersisted && circuitEvent && !events.some((existing) => existing.run_id === circuitEvent.run_id)
      ? [...events, circuitEvent]
      : events;
  const state = currentCircuitState(combinedEvents, event.at);
  const body = renderInfrastructureHandoffComment(event, state);
  try {
    if (DRY_RUN) console.log(body);
    else postIssueComment(PR, body, deadlines.commentDeadlineMs);
  } catch (error) {
    console.error(`::warning::发布 Review 降级评论失败：${errorText(error)}`);
  }
}

export function renderInfrastructureHandoffComment(event, state) {
  const circuitLine = state.open
    ? `\n\n已达到基础设施失败阈值，自动触发将熔断至 **${state.openUntil}**；可信成员评论精确命令 \`/review\` 可手动旁路重试。`
    : "";
  return `## ⚠️ Review Action 基础设施降级\n\n请忽略本次 Review Action 结果：这是基础设施失败，不是代码 finding。改走 agent 自有博弈式 review 流程并向用户反馈；本 workflow 已成功降级退出，不中断任务。\n\n失败阶段：\`${event.phase}\`。${circuitLine}\n\n${renderHiddenMarker(HANDOFF_MARKER, event)}`;
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
  return `## ⚡ Review Action 自动熔断跳过\n\nreviewer 基础设施熔断中，请忽略本次 Review Action 基础设施 gate：这不是代码 finding。改走 agent 自有博弈式 review 流程并向用户反馈；本次自动触发已快速跳过并成功退出，不中断任务且不影响其他 CI。\n\n熔断截止：**${state.openUntil}**。可信成员评论精确命令 \`/review\` 可手动旁路重试。\n\n${renderHiddenMarker(HANDOFF_MARKER, marker)}`;
}

function postIssueComment(issue, body, deadlineMs = Date.now() + GH_TIMEOUT_MS) {
  const repo = resolveRepository();
  gh(["api", `repos/${repo}/issues/${issue}/comments`, "-f", `body=${body}`], circuitGhTimeout(deadlineMs));
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
function sleepSync(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}
export function circuitGhTimeout(deadlineMs, nowMs = Date.now()) {
  const remainingMs = Math.floor(deadlineMs - nowMs);
  if (!Number.isFinite(remainingMs) || remainingMs <= 0) throw new Error("Review 熔断操作预算已耗尽。");
  return Math.max(1, Math.min(GH_TIMEOUT_MS, remainingMs));
}
function gh(args, timeoutMs = GH_TIMEOUT_MS) {
  const timeout = Math.max(1, Math.min(GH_TIMEOUT_MS, Math.floor(timeoutMs)));
  return execFileSync("gh", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024, timeout });
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
