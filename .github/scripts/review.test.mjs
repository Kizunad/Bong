// Review v3 纯逻辑测试 —— `node --test .github/scripts/review.test.mjs`

import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  applyPlanIntentGate,
  boundedAttemptTimeout,
  buildChatCompletionsEndpoint,
  buildCircuitStateSearchQuery,
  circuitGhTimeout,
  circuitOperationDeadlines,
  classifyReviewRun,
  classifyWorkflowFinalization,
  codexRetryDelayMs,
  codexFailureText,
  decideGate,
  decideReviewGate,
  evaluateCircuit,
  ensureCircuitStateIssues,
  excerptLog,
  extractJSON,
  extractChatCompletionsOutputText,
  findCircuitStateIssues,
  findPlanName,
  isCircuitBypassTrigger,
  isRetryableCodexFailure,
  isSuccessfulCodexResponse,
  isZeroConfidenceWithoutCodeFindings,
  localPrDiff,
  loadPrContext,
  mergeFindings,
  normalizePlanStatus,
  normalizeResult,
  normalizeVote,
  normalizeCircuitEvent,
  parseGitHubJsonLines,
  parseHiddenMarkers,
  parseTrustedCircuitEvents,
  redactRuntimeSecrets,
  readWorkspaceRegularFile,
  renderComment,
  requestChatCompletions,
  runReviewPanel,
  renderCircuitSkipComment,
  renderHiddenMarker,
  renderInfrastructureHandoffComment,
  reviewFindingResults,
  resolveCircuitStateIssueNumbers,
  selectCircuitStateIssues,
} from "./review.mjs";

const reviewer = { id: "A", name: "Plan 原意核查" };

const infraResult = {
  execution_failure: true,
  vote: "REQUEST_CHANGES",
  confidence: 0,
  findings: [{ file: ".github/scripts/review.mjs", title: "Codex reviewer initial-A 执行失败" }],
};
const realRequestChanges = {
  vote: "REQUEST_CHANGES",
  confidence: 95,
  findings: [{ file: "server/src/main.rs", title: "真实代码缺陷" }],
};
const reviewScript = fileURLToPath(new URL("./review.mjs", import.meta.url));
const textEncoder = new TextEncoder();

function chatSseChunk(content, finish_reason = null, lineEnding = "\n") {
  const delta = content === null ? { role: "assistant", content: null } : { content };
  return `data: ${JSON.stringify({ choices: [{ delta, finish_reason }] })}${lineEnding}${lineEnding}`;
}

function chatSseDone(lineEnding = "\n") {
  return `data: [DONE]${lineEnding}${lineEnding}`;
}
function byteStream(chunks, { neverClose = false, cancel } = {}) {
  return new ReadableStream({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(chunk instanceof Uint8Array ? chunk : textEncoder.encode(chunk));
      if (!neverClose) controller.close();
    },
    cancel(reason) {
      cancel?.(reason);
    },
  });
}

function sseResponse(chunks, options = {}) {
  return {
    ok: true,
    status: 200,
    statusText: "OK",
    body: byteStream(chunks, options),
  };
}

function runReviewCommand(
  command,
  { env = {}, ghScript = "process.exit(91);", outcome = null, comment = null } = {},
) {
  const directory = mkdtempSync(join(tmpdir(), "bong-review-command-"));
  const ghPath = join(directory, "gh");
  const outputPath = join(directory, "github-output");
  const outcomePath = join(directory, "review-outcome.json");
  const commentPath = join(directory, "review.md");
  const ghLogPath = join(directory, "gh.log");
  writeFileSync(ghPath, `#!/usr/bin/env node\n${ghScript}\n`);
  chmodSync(ghPath, 0o755);
  if (outcome) writeFileSync(outcomePath, `${JSON.stringify(outcome)}\n`);
  if (comment !== null) writeFileSync(commentPath, String(comment));

  try {
    const result = spawnSync(process.execPath, [reviewScript, command], {
      encoding: "utf8",
      timeout: 30_000,
      env: {
        ...process.env,
        PATH: `${directory}:${process.env.PATH || ""}`,
        PR_NUMBER: "1148",
        GITHUB_REPOSITORY: "Kizunad/Bong",
        GITHUB_RUN_ID: "9001",
        GITHUB_OUTPUT: outputPath,
        GH_TEST_LOG: ghLogPath,
        REVIEW_OUTCOME_FILE: outcomePath,
        REVIEW_COMMENT_FILE: commentPath,
        REVIEW_CIRCUIT_SEARCH_INTERVAL_MS: "1",
        REVIEW_NOW: "2026-07-10T00:20:00.000Z",
        ...env,
      },
    });
    return {
      ...result,
      githubOutput: existsSync(outputPath) ? readFileSync(outputPath, "utf8") : "",
      reviewOutcome: existsSync(outcomePath) ? JSON.parse(readFileSync(outcomePath, "utf8")) : null,
      reviewComment: existsSync(commentPath) ? readFileSync(commentPath, "utf8") : "",
      ghLog: existsSync(ghLogPath) ? readFileSync(ghLogPath, "utf8") : "",
    };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function openCircuitGhScript() {
  const stateIssue = {
    number: 1149,
    title: "[automation] Review infrastructure circuit state",
    body: "<!-- bong-review-circuit-state:v1 -->",
    user: { login: "github-actions[bot]", type: "Bot" },
  };
  const comments = ["00:00", "00:10", "00:20"].map((time, index) => ({
    body: renderHiddenMarker("bong-review-circuit", {
      v: 1,
      kind: "infra_failure",
      run_id: String(100 + index),
      at: `2026-07-10T${time}:00.000Z`,
    }),
    user: { login: "github-actions[bot]", type: "Bot" },
  }));
  return `
const args = process.argv.slice(2).join(" ");
if (args.includes("search/issues")) process.stdout.write(${JSON.stringify(JSON.stringify(stateIssue))});
else if (args.includes("issues/1149/comments")) process.stdout.write(${JSON.stringify(comments.map(JSON.stringify).join("\n"))});
else if (args.includes("issues/1148/comments")) process.stdout.write("{}");
else process.exit(92);
`;
}

function recordingGhScript(stateComments = []) {
  const stateIssue = {
    number: 1149,
    title: "[automation] Review infrastructure circuit state",
    body: "<!-- bong-review-circuit-state:v1 -->",
    user: { login: "github-actions[bot]", type: "Bot" },
  };
  return `
const fs = require("node:fs");
const args = process.argv.slice(2);
const joined = args.join(" ");
const log = (entry) => fs.appendFileSync(process.env.GH_TEST_LOG, JSON.stringify(entry) + "\\n");
log({ args });
if (args[0] === "pr" && args[1] === "view") {
  process.stdout.write(${JSON.stringify(JSON.stringify({ title: "review infra", body: "", headRefName: "fix/review", files: [] }))});
} else if (args[0] === "pr" && args[1] === "diff") {
  process.stdout.write("");
} else if (args[0] === "pr" && args[1] === "comment") {
  const bodyFile = args[args.indexOf("--body-file") + 1];
  log({ kind: "pr_comment", body: fs.readFileSync(bodyFile, "utf8") });
  process.stdout.write("{}");
} else if (joined.includes("search/issues")) {
  process.stdout.write(${JSON.stringify(JSON.stringify(stateIssue))});
} else if (joined.includes("issues/1149/comments?")) {
  process.stdout.write(${JSON.stringify(stateComments.map(JSON.stringify).join("\n"))});
} else if (joined.includes("issues/1149/comments")) {
  log({ kind: "state_comment", body: args.find((arg) => arg.startsWith("body="))?.slice(5) || "" });
  process.stdout.write("{}");
} else if (joined.includes("issues/1148/comments")) {
  log({ kind: "handoff_comment", body: args.find((arg) => arg.startsWith("body="))?.slice(5) || "" });
  process.stdout.write("{}");
} else {
  process.exit(92);
}
`;
}

function reviewContext() {
  return {
    pr: "1148",
    title: "review infra",
    body: "",
    headRefName: "fix/review",
    fileList: "- .github/scripts/review.mjs",
    changedFiles: 1,
    changedLines: 1,
    diff: "diff --git a/review.mjs b/review.mjs",
    diffTruncated: false,
    plan: null,
  };
}

function reviewerFixtureRunner(mode = "approve") {
  let call = 0;
  return async (_prompt, label) => {
    const previous = call++;
    if (mode === "infra") {
      return {
        raw: JSON.stringify({
          vote: "REQUEST_CHANGES",
          confidence: 0,
          plan_intent: { status: "unclear", reason: "provider 503", missing: [] },
          summary: "provider 503",
          findings: [{
            severity: "major",
            file: ".github/scripts/review.mjs",
            line: "0",
            title: `Codex reviewer ${label} 执行失败`,
          }],
        }),
        executionFailure: true,
      };
    }
    if (mode === "zero_empty") {
      return {
        raw: JSON.stringify({
          vote: "REQUEST_CHANGES",
          confidence: 0,
          plan_intent: { status: "unclear", reason: "空结果", missing: [] },
          summary: "zero confidence",
          findings: [],
        }),
        executionFailure: true,
      };
    }
    const requestChanges =
      (mode === "tie" && previous >= 6) ||
      ((mode === "three_one" || mode === "three_one_empty") && previous >= 7) ||
      (mode === "first_finding_withdrawn" && previous === 0);
    return {
      raw: JSON.stringify({
        reviewer: label,
        vote: requestChanges ? "REQUEST_CHANGES" : "APPROVE",
        confidence: 95,
        plan_intent: { status: "not_plan", reason: "非 plan PR", missing: [] },
        summary: requestChanges ? "要求修改" : "通过",
        findings:
          requestChanges && mode !== "three_one_empty"
            ? [{ severity: "major", file: "server/src/main.rs", line: "1", title: "真实问题" }]
            : [],
      }),
      executionFailure: false,
    };
  };
}

async function evaluatePanel(mode) {
  const context = reviewContext();
  const panel = await runReviewPanel(context, reviewerFixtureRunner(mode));
  const gate = decideReviewGate(panel.finalRound, [...panel.firstRound, ...panel.finalRound]);
  return { ...panel, gate, body: renderComment(context, panel.firstRound, panel.finalRound, gate) };
}

function workflowJob(workflow, name) {
  const jobs = workflow.slice(workflow.indexOf("\njobs:"));
  const marker = `\n  ${name}:\n`;
  const start = jobs.indexOf(marker);
  if (start < 0) return "";
  const rest = jobs.slice(start + marker.length);
  const next = rest.search(/\n  [a-z][a-z0-9_-]*:\n/);
  return next < 0 ? rest : rest.slice(0, next);
}

test("hidden marker: 可往返解析并忽略普通/损坏评论", () => {
  const marker = { v: 1, kind: "infra_failure", at: "2026-07-10T00:00:00.000Z", reason: "429 } -- upstream" };
  const body = `说明\n${renderHiddenMarker("bong-review-circuit", marker)}`;
  const parsed = parseHiddenMarkers([{ body }, { body: "普通评论" }, { body: "<!-- bong-review-circuit {bad} -->" }]);
  assert.equal(parsed.length, 1);
  assert.equal(parsed[0].kind, "infra_failure");
  assert.equal(parsed[0].reason, "429 } —— upstream");
});


test("handoff marker: 中文降级评论可被主 agent 机器识别", () => {
  const event = { v: 1, kind: "infra_failure", at: "2026-07-10T00:00:00.000Z", phase: "reviewer_execution" };
  const body = renderInfrastructureHandoffComment(event, { open: true, openUntil: "2026-07-10T01:00:00.000Z" });
  assert.match(body, /请忽略本次 Review Action 结果/);
  assert.match(body, /基础设施失败，不是代码 finding/);
  assert.match(body, /改走 agent 自有博弈式 review 流程并向用户反馈/);
  assert.match(body, /成功降级退出，不中断任务/);
  assert.match(body, /\/review/);
  assert.deepEqual(parseHiddenMarkers(body, "bong-review-handoff"), [event]);
});

test("熔断跳过评论: 明示 agent 交接、截止时间、成功退出和 /review 手动旁路", () => {
  const body = renderCircuitSkipComment({ open: true, openUntil: "2026-07-10T01:00:00.000Z" });
  assert.match(body, /忽略本次 Review Action 基础设施 gate/);
  assert.match(body, /不是代码 finding/);
  assert.match(body, /改走 agent 自有博弈式 review 流程并向用户反馈/);
  assert.match(body, /快速跳过并成功退出，不中断任务且不影响其他 CI/);
  assert.match(body, /2026-07-10T01:00:00.000Z/);
  assert.match(body, /\/review/);
  assert.equal(parseHiddenMarkers(body, "bong-review-handoff")[0].kind, "circuit_skip");
});

test("trusted circuit events: 只接受 github-actions bot、单评论单 marker，并按 run_id 去重", () => {
  const marker = (runId, at) => renderHiddenMarker("bong-review-circuit", {
    v: 1,
    kind: "infra_failure",
    run_id: runId,
    at,
  });
  const comments = [
    { body: marker("100", "2026-07-10T00:00:00.000Z"), user: { login: "attacker", type: "User" } },
    {
      body: `${marker("101", "2026-07-10T00:01:00.000Z")}\n${marker("102", "2026-07-10T00:02:00.000Z")}`,
      user: { login: "github-actions[bot]", type: "Bot" },
    },
    { body: marker("101", "2026-07-10T00:03:00.000Z"), user: { login: "github-actions[bot]", type: "Bot" } },
    { body: marker("103", "2026-07-10T00:04:00.000Z"), user: { login: "github-actions[bot]", type: "User" } },
    { body: marker("104", "2026-07-10T00:05:00.000Z"), user: { login: "github-actions[bot]", type: "Bot" } },
  ];
  assert.deepEqual(parseTrustedCircuitEvents(comments).map((event) => event.run_id), ["101", "104"]);
});


test("circuit event validation: 当前轮与跨 run 对空/非法 run_id 使用同一规则", () => {
  const base = { v: 1, kind: "infra_failure", at: "2026-07-10T00:00:00.000Z" };
  assert.equal(normalizeCircuitEvent({ ...base, run_id: "" }), null);
  assert.equal(normalizeCircuitEvent({ ...base, run_id: "abc" }), null);
  assert.equal(normalizeCircuitEvent({ ...base, run_id: "123" }).run_id, "123");
  for (const invalid of [
    { ...base, v: 2, run_id: "123" },
    { ...base, kind: "gate_failure", run_id: "123" },
    { ...base, at: null, run_id: "123" },
    { ...base, at: false, run_id: "123" },
    { ...base, at: 1_783_641_600_000, run_id: "123" },
    { ...base, at: "2026-07-10", run_id: "123" },
    { ...base, at: "2026-02-30T00:00:00.000Z", run_id: "123" },
    { ...base, at: "2026-07-10T00:00:00Z", run_id: "123" },
  ]) {
    assert.equal(normalizeCircuitEvent(invalid), null, `非法事件不得计数：${JSON.stringify(invalid)}`);
  }
});

test("state issue concurrency: 两个并发创建者重查后选择相同 canonical issue", () => {
  assert.deepEqual(resolveCircuitStateIssueNumbers("20", ["21", "20"]), ["20", "21"]);
  assert.deepEqual(resolveCircuitStateIssueNumbers("21", ["20", "21"]), ["20", "21"]);
  assert.deepEqual(resolveCircuitStateIssueNumbers("20", ["21"]), ["20", "21"], "搜索索引只看见对方副本时仍须并入本轮新建号");
  assert.deepEqual(resolveCircuitStateIssueNumbers("21", ["20"]), ["20", "21"], "并发双方必须收敛到相同排序集合");
  assert.deepEqual(resolveCircuitStateIssueNumbers("21", ["21", "bad", "20", "21"]), ["20", "21"]);
  assert.deepEqual(resolveCircuitStateIssueNumbers("20", []), ["20"]);
  assert.throws(() => resolveCircuitStateIssueNumbers("bad", ["20"]), /issue number 非法/);
});
test("state issues: 聚合合法重复状态 issue，并拒绝伪标题、非 bot、缺 marker 与 PR", () => {
  const body = "<!-- bong-review-circuit-state:v1 -->";
  const bot = { login: "github-actions[bot]", type: "Bot" };
  assert.deepEqual(
    selectCircuitStateIssues([
      { number: 20, title: "[automation] Review infrastructure circuit state", body, user: bot, pull_request: null },
      { number: 3, title: "[automation] Review infrastructure circuit state", body, user: bot, state: "closed" },
      { number: 20, title: "[automation] Review infrastructure circuit state", body, user: bot },
      { number: 1, title: "其他", body, user: bot },
      { number: 5, title: "[automation] Review infrastructure circuit state injected", body, user: bot },
      { number: 6, title: '[automation] Review infrastructure circuit state" is:pr', body, user: bot },
      { number: 2, title: "[automation] Review infrastructure circuit state", body, user: { login: "attacker", type: "User" } },
      { number: 4, title: "[automation] Review infrastructure circuit state", body, user: bot, pull_request: {} },
      { number: 7, title: "[automation] Review infrastructure circuit state", body: "marker missing", user: bot },
      { number: "not-a-number", title: "[automation] Review infrastructure circuit state", body, user: bot },
    ]),
    ["3", "20"],
  );
});

test("state issue search query: 固定 repo/title/is:issue，拒绝 repo 与 title 查询注入", () => {
  assert.equal(
    buildCircuitStateSearchQuery("Kizunad/Bong"),
    'repo:Kizunad/Bong is:issue in:title "[automation] Review infrastructure circuit state"',
  );
  for (const repo of ["", "Kizunad", "Kizunad/Bong is:pr", "Kizunad/Bong\norg:attacker", "Kizunad/Bong/extra"]) {
    assert.throws(() => buildCircuitStateSearchQuery(repo), /GITHUB_REPOSITORY 非法/);
  }
  for (const title of [
    '[automation] Review infrastructure circuit state" is:pr',
    "[automation] Review infrastructure circuit state\nrepo:attacker/repo",
    "其他状态标题",
  ]) {
    assert.throws(() => buildCircuitStateSearchQuery("Kizunad/Bong", title), /title 非法/);
  }
});

test("GitHub pagination parsing: 空结果、多页 NDJSON 与损坏响应边界", () => {
  assert.deepEqual(parseGitHubJsonLines(""), []);
  assert.deepEqual(parseGitHubJsonLines('\n{"number":3}\r\n{"number":20}\n'), [{ number: 3 }, { number: 20 }]);
  assert.throws(() => parseGitHubJsonLines('{"number":3}\nnot-json'), SyntaxError);
});

test("findCircuitStateIssues: 四轮 Search 均限定标题并累积 duplicate resolution，绝不全仓扫描", () => {
  const body = "<!-- bong-review-circuit-state:v1 -->";
  const bot = { login: "github-actions[bot]", type: "Bot" };
  let calls = 0;
  let calledArgs;
  let nowMs = 0;
  const waits = [];
  const found = findCircuitStateIssues(
    "Kizunad/Bong",
    (args) => {
      calls += 1;
      calledArgs = args;
      return [
        { number: 20, title: "[automation] Review infrastructure circuit state", body, user: bot },
        { number: 3, title: "[automation] Review infrastructure circuit state", body, user: bot },
        { number: 4, title: "[automation] Review infrastructure circuit state", body, user: bot, pull_request: {} },
      ]
        .map(JSON.stringify)
        .join("\n");
    },
    (ms) => { waits.push(ms); nowMs += ms; },
    { now: () => nowMs, deadlineMs: 120_000 },
  );

  assert.deepEqual(found, ["3", "20"]);
  assert.equal(calls, 4, "必须走完有限观察窗，避免漏掉稍后可见的并发副本");
  assert.deepEqual(waits, [15_000, 15_000, 15_000]);
  assert.deepEqual(calledArgs, [
    "api",
    "--paginate",
    "--method",
    "GET",
    "search/issues",
    "-f",
    'q=repo:Kizunad/Bong is:issue in:title "[automation] Review infrastructure circuit state"',
    "-f",
    "per_page=100",
    "--jq",
    ".items[]",
  ]);
  assert.doesNotMatch(calledArgs.join(" "), /repos\/Kizunad\/Bong\/issues\?|state=all|is:pr/);
});

test("findCircuitStateIssues: 最终一致性期间累积部分可见结果且不丢已解析状态", () => {
  const body = "<!-- bong-review-circuit-state:v1 -->";
  const bot = { login: "github-actions[bot]", type: "Bot" };
  const issue = (number) => JSON.stringify({
    number,
    title: "[automation] Review infrastructure circuit state",
    body,
    user: bot,
  });
  const outputs = [issue(20), "", issue(21), issue(20)];
  const waits = [];
  let calls = 0;
  let nowMs = 0;

  const found = findCircuitStateIssues(
    "Kizunad/Bong",
    () => outputs[calls++] ?? "",
    (ms) => { waits.push(ms); nowMs += ms; },
    { now: () => nowMs, deadlineMs: 120_000 },
  );

  assert.deepEqual(found, ["20", "21"]);
  assert.equal(calls, 4);
  assert.deepEqual(waits, [15_000, 15_000, 15_000]);
});

test("findCircuitStateIssues: 持续空结果最多四次请求后返回空集，维持上层 fail-open", () => {
  const waits = [];
  let calls = 0;
  let nowMs = 0;
  const found = findCircuitStateIssues(
    "Kizunad/Bong",
    () => {
      calls += 1;
      return "";
    },
    (ms) => { waits.push(ms); nowMs += ms; },
    { now: () => nowMs, deadlineMs: 120_000 },
  );

  assert.deepEqual(found, []);
  assert.equal(calls, 4, "Search 请求数必须有硬上限");
  assert.deepEqual(waits, [15_000, 15_000, 15_000]);
});

test("findCircuitStateIssues: API 与 JSON 错误直接上抛，不把确定性失败伪装成索引延迟", () => {
  const waits = [];
  assert.throws(
    () => findCircuitStateIssues("Kizunad/Bong", () => { throw new Error("HTTP 403"); }, (ms) => waits.push(ms)),
    /HTTP 403/,
  );
  assert.throws(() => findCircuitStateIssues("Kizunad/Bong", () => "not-json", (ms) => waits.push(ms)), SyntaxError);
  assert.deepEqual(waits, []);
});

test("findCircuitStateIssues: 非法 Search 次数与间隔在发请求前拒绝", () => {
  let calls = 0;
  const runGh = () => { calls += 1; return ""; };
  for (const timing of [
    { searchAttempts: 0 },
    { searchAttempts: 1.5 },
    { searchIntervalMs: 0 },
    { searchIntervalMs: Number.NaN },
  ]) {
    assert.throws(() => findCircuitStateIssues("Kizunad/Bong", runGh, () => {}, timing), /节流配置非法/);
  }
  assert.equal(calls, 0);
});

test("circuit deadline: 单次 gh timeout 受 30 秒与共享剩余预算双重约束", () => {
  assert.equal(circuitGhTimeout(120_000, 0), 30_000);
  assert.equal(circuitGhTimeout(120_000, 100_001), 19_999);
  assert.equal(circuitGhTimeout(120_000, 119_999), 1);
  assert.throws(() => circuitGhTimeout(120_000, 120_000), /预算已耗尽/);
  assert.throws(() => circuitGhTimeout(Number.NaN, 0), /预算已耗尽/);
});

test("handoff deadline: 状态预算耗尽后仍保留独立评论机会", () => {
  const deadlines = circuitOperationDeadlines(0, 120_000, 30_000);
  assert.deepEqual(deadlines, { stateDeadlineMs: 120_000, commentDeadlineMs: 150_000 });
  assert.equal(circuitGhTimeout(deadlines.commentDeadlineMs, deadlines.stateDeadlineMs), 30_000);
  assert.throws(() => circuitOperationDeadlines(0, 0, 30_000), /预算非法/);
  assert.throws(() => circuitOperationDeadlines(Number.NaN, 120_000, 30_000), /预算非法/);
});

test("ensureCircuitStateIssues: 创建前后 Search 共用 120 秒 deadline，不突破三分钟预检", () => {
  let nowMs = 0;
  const calls = [];
  const runGh = (args, timeoutMs) => {
    calls.push({ endpoint: args[4] === "search/issues" ? "search" : "create", timeoutMs });
    nowMs += timeoutMs;
    return "";
  };

  assert.throws(
    () => ensureCircuitStateIssues({
      repo: "Kizunad/Bong",
      runGh,
      wait: (ms) => { nowMs += ms; },
      now: () => nowMs,
      deadlineMs: 120_000,
    }),
    /预算已耗尽/,
  );
  assert.equal(nowMs, 120_000);
  assert.equal(calls.length, 3, "15 秒退避与 API timeout 耗尽预算后不得继续创建 issue 或启动第二轮 Search");
  assert.deepEqual(calls.map((call) => call.endpoint), ["search", "search", "search"]);
  assert.ok(calls.every((call) => call.timeoutMs <= 30_000));
});

test("ensureCircuitStateIssues: 快速空查询后创建并在剩余 deadline 内合并延迟可见副本", () => {
  const body = "<!-- bong-review-circuit-state:v1 -->";
  const bot = { login: "github-actions[bot]", type: "Bot" };
  let nowMs = 0;
  let searchCalls = 0;
  const searchStarts = [];
  const runGh = (args, timeoutMs) => {
    assert.ok(timeoutMs > 0 && timeoutMs <= 30_000);
    if (args[1] !== `repos/Kizunad/Bong/issues`) searchStarts.push(nowMs);
    nowMs += 100;
    if (args[1] === `repos/Kizunad/Bong/issues`) return JSON.stringify({ number: 20 });
    searchCalls += 1;
    if (searchCalls < 5) return "";
    return JSON.stringify({ number: 21, title: "[automation] Review infrastructure circuit state", body, user: bot });
  };

  const found = ensureCircuitStateIssues({
    repo: "Kizunad/Bong",
    runGh,
    wait: (ms) => { nowMs += ms; },
    now: () => nowMs,
    deadlineMs: 120_000,
  });
  assert.deepEqual(found, ["20", "21"]);
  assert.equal(searchCalls, 8, "创建前后各四次查询都必须完成并累积结果");
  assert.ok(
    searchStarts.slice(1).every((start, index) => start - searchStarts[index] >= 15_000),
    `所有 Search 起点必须至少间隔 15 秒：${searchStarts.join(",")}`,
  );
  assert.ok(nowMs < 120_000);
});

test("review 总预算: 单次 timeout 被剩余预算截断，并预留清理时间", () => {
  assert.equal(boundedAttemptTimeout(900_000, 1_000_000), 820_000);
  assert.equal(boundedAttemptTimeout(900_000, 180_000), 0);
  assert.equal(boundedAttemptTimeout(900_000, 60_000, 15_000), 45_000);
});

test("local PR diff: fork head 通过 PR ref fetch、锁定 metadata OID，并以 merge-base 生成三点语义 diff", () => {
  const directory = mkdtempSync(join(tmpdir(), "bong-review-git-"));
  const gitPath = join(directory, "git");
  const logPath = join(directory, "git.log");
  const fetchedBasePath = join(directory, "fetched-base");
  const fetchedHeadPath = join(directory, "fetched-head");
  const baseOid = "a".repeat(40);
  const headOid = "b".repeat(40);
  const mergeBase = "c".repeat(40);
  writeFileSync(
    gitPath,
    `#!/usr/bin/env node
const fs = require("node:fs");
const args = process.argv.slice(2);
fs.appendFileSync(process.env.GIT_TEST_LOG, JSON.stringify(args) + "\\n");
const baseOid = ${JSON.stringify(baseOid)};
const headOid = ${JSON.stringify(headOid)};
const mergeBase = ${JSON.stringify(mergeBase)};
if (args[0] === "fetch") {
  if (args[3] === "+refs/heads/main:refs/review/pr-42-base") {
    fs.writeFileSync(process.env.GIT_TEST_FETCHED_BASE, "yes");
  } else if (args[3] === "+refs/pull/42/head:refs/review/pr-42-head") {
    fs.writeFileSync(process.env.GIT_TEST_FETCHED_HEAD, "yes");
  } else {
    process.exit(2);
  }
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-base^{commit}") {
  if (!fs.existsSync(process.env.GIT_TEST_FETCHED_BASE)) process.exit(1);
  process.stdout.write(baseOid + "\\n");
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-head^{commit}") {
  if (!fs.existsSync(process.env.GIT_TEST_FETCHED_HEAD)) process.exit(1);
  process.stdout.write(headOid + "\\n");
} else if (args[0] === "merge-base") {
  if (args[1] !== baseOid || args[2] !== headOid) process.exit(4);
  process.stdout.write(mergeBase + "\\n");
} else if (args[0] === "diff") {
  if (args[2] !== mergeBase || args[3] !== headOid) process.exit(5);
  process.stdout.write("diff --git a/fork.txt b/fork.txt\\n");
} else {
  process.exit(3);
}
`,
  );
  chmodSync(gitPath, 0o755);
  const previousPath = process.env.PATH;
  const previousLog = process.env.GIT_TEST_LOG;
  process.env.PATH = `${directory}:${previousPath || ""}`;
  process.env.GIT_TEST_LOG = logPath;
  process.env.GIT_TEST_FETCHED_BASE = fetchedBasePath;
  process.env.GIT_TEST_FETCHED_HEAD = fetchedHeadPath;
  try {
    assert.equal(
      localPrDiff("42", {
        baseRefOid: baseOid,
        baseRefName: "main",
        headRefOid: headOid,
        headRefName: "feature",
      }),
      "diff --git a/fork.txt b/fork.txt\n",
    );
    const calls = readFileSync(logPath, "utf8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.deepEqual(calls.filter((args) => args[0] === "fetch"), [
      ["fetch", "--no-tags", "origin", "+refs/heads/main:refs/review/pr-42-base"],
      ["fetch", "--no-tags", "origin", "+refs/pull/42/head:refs/review/pr-42-head"],
    ]);
    assert.deepEqual(calls.filter((args) => args[0] === "rev-parse"), [
      ["rev-parse", "--verify", "refs/review/pr-42-base^{commit}"],
      ["rev-parse", "--verify", "refs/review/pr-42-head^{commit}"],
    ]);
    assert.deepEqual(calls.find((args) => args[0] === "merge-base"), ["merge-base", baseOid, headOid]);
    assert.deepEqual(calls.find((args) => args[0] === "diff"), ["diff", "--no-ext-diff", mergeBase, headOid]);
    assert.equal(calls.some((args) => args.includes("feature")), false, "fork fallback 不得 fetch base origin 的同名 branch");
  } finally {
    if (previousPath === undefined) delete process.env.PATH;
    else process.env.PATH = previousPath;
    if (previousLog === undefined) delete process.env.GIT_TEST_LOG;
    else process.env.GIT_TEST_LOG = previousLog;
    delete process.env.GIT_TEST_FETCHED_BASE;
    delete process.env.GIT_TEST_FETCHED_HEAD;
    rmSync(directory, { recursive: true, force: true });
  }
});

test("local PR diff: script-owned ref 用强制 refspec 刷新 force-push 后的 stale head", () => {
  const directory = mkdtempSync(join(tmpdir(), "bong-review-git-force-push-"));
  const gitPath = join(directory, "git");
  const logPath = join(directory, "git.log");
  const refStatePath = join(directory, "head-ref");
  const baseOid = "a".repeat(40);
  const headOid = "b".repeat(40);
  const mergeBase = "c".repeat(40);
  const staleHeadOid = "d".repeat(40);
  writeFileSync(refStatePath, `${staleHeadOid}\n`);
  writeFileSync(
    gitPath,
    `#!/usr/bin/env node
const fs = require("node:fs");
const args = process.argv.slice(2);
const baseOid = ${JSON.stringify(baseOid)};
const headOid = ${JSON.stringify(headOid)};
const mergeBase = ${JSON.stringify(mergeBase)};
const staleHeadOid = ${JSON.stringify(staleHeadOid)};
const refStatePath = process.env.GIT_TEST_REF_STATE;
fs.appendFileSync(process.env.GIT_TEST_LOG, JSON.stringify(args) + "\\n");
if (args[0] === "fetch") {
  if (args[3] === "+refs/heads/main:refs/review/pr-42-base") {
    process.exit(0);
  }
  if (args[3] === "+refs/pull/42/head:refs/review/pr-42-head") {
    if (fs.readFileSync(refStatePath, "utf8").trim() !== staleHeadOid) process.exit(2);
    fs.writeFileSync(refStatePath, headOid + "\\n");
    process.exit(0);
  }
  process.stderr.write("unexpected refspec\\n");
  process.exit(3);
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-base^{commit}") {
  process.stdout.write(baseOid + "\\n");
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-head^{commit}") {
  process.stdout.write(fs.readFileSync(refStatePath, "utf8").trim() + "\\n");
} else if (args[0] === "merge-base") {
  if (args[1] !== baseOid || args[2] !== headOid) process.exit(4);
  process.stdout.write(mergeBase + "\\n");
} else if (args[0] === "diff") {
  if (args[2] !== mergeBase || args[3] !== headOid) process.exit(5);
  process.stdout.write("diff --git a/force-push.txt b/force-push.txt\\n");
} else {
  process.exit(6);
}
`,
  );
  chmodSync(gitPath, 0o755);
  const previousPath = process.env.PATH;
  const previousLog = process.env.GIT_TEST_LOG;
  const previousRefState = process.env.GIT_TEST_REF_STATE;
  process.env.PATH = `${directory}:${previousPath || ""}`;
  process.env.GIT_TEST_LOG = logPath;
  process.env.GIT_TEST_REF_STATE = refStatePath;
  try {
    assert.equal(
      localPrDiff("42", {
        baseRefOid: baseOid,
        baseRefName: "main",
        headRefOid: headOid,
        headRefName: "feature",
      }),
      "diff --git a/force-push.txt b/force-push.txt\n",
    );
    assert.equal(
      readFileSync(refStatePath, "utf8").trim(),
      headOid,
      "force-push 后必须把 script-owned ref 更新到 metadata head OID",
    );
    const calls = readFileSync(logPath, "utf8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.deepEqual(calls.slice(0, 4), [
      ["fetch", "--no-tags", "origin", "+refs/heads/main:refs/review/pr-42-base"],
      ["fetch", "--no-tags", "origin", "+refs/pull/42/head:refs/review/pr-42-head"],
      ["rev-parse", "--verify", "refs/review/pr-42-base^{commit}"],
      ["rev-parse", "--verify", "refs/review/pr-42-head^{commit}"],
    ]);
  } finally {
    if (previousPath === undefined) delete process.env.PATH;
    else process.env.PATH = previousPath;
    if (previousLog === undefined) delete process.env.GIT_TEST_LOG;
    else process.env.GIT_TEST_LOG = previousLog;
    if (previousRefState === undefined) delete process.env.GIT_TEST_REF_STATE;
    else process.env.GIT_TEST_REF_STATE = previousRefState;
    rmSync(directory, { recursive: true, force: true });
  }
});

test("local PR diff: force-push 后即使 metadata OID 对象已在本地，也始终刷新并从 refs 核验", () => {
  const directory = mkdtempSync(join(tmpdir(), "bong-review-git-existing-object-"));
  const gitPath = join(directory, "git");
  const logPath = join(directory, "git.log");
  const baseOid = "a".repeat(40);
  const headOid = "b".repeat(40);
  const staleHeadOid = "d".repeat(40);
  const refStatePath = join(directory, "head-ref");
  writeFileSync(refStatePath, `${staleHeadOid}\n`);
  writeFileSync(gitPath, `#!/usr/bin/env node
const fs = require("node:fs");
const args = process.argv.slice(2);
const baseOid = ${JSON.stringify(baseOid)};
const staleHeadOid = ${JSON.stringify(staleHeadOid)};
const refStatePath = ${JSON.stringify(refStatePath)};
fs.appendFileSync(${JSON.stringify(logPath)}, JSON.stringify(args) + "\\n");
if (args[0] === "fetch") {
  if (args[3] === "+refs/heads/main:refs/review/pr-42-base") process.exit(0);
  if (args[3] === "+refs/pull/42/head:refs/review/pr-42-head") process.exit(0);
  process.exit(2);
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-base^{commit}") {
  process.stdout.write(baseOid + "\\n");
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-head^{commit}") {
  process.stdout.write(fs.readFileSync(refStatePath, "utf8"));
} else if (args[0] === "merge-base" || args[0] === "diff") {
  process.exit(7);
} else {
  process.exit(3);
}
`);
  chmodSync(gitPath, 0o755);
  const previousPath = process.env.PATH;
  process.env.PATH = `${directory}:${previousPath || ""}`;
  try {
    assert.throws(
      () => localPrDiff("42", {
        baseRefOid: baseOid,
        baseRefName: "main",
        headRefOid: headOid,
        headRefName: "feature",
      }),
      new RegExp(`PR head OID 漂移：expected=${headOid} actual=${staleHeadOid}`),
    );
    const calls = readFileSync(logPath, "utf8").trim().split("\n").map((line) => JSON.parse(line));
    assert.deepEqual(calls.slice(0, 4), [
      ["fetch", "--no-tags", "origin", "+refs/heads/main:refs/review/pr-42-base"],
      ["fetch", "--no-tags", "origin", "+refs/pull/42/head:refs/review/pr-42-head"],
      ["rev-parse", "--verify", "refs/review/pr-42-base^{commit}"],
      ["rev-parse", "--verify", "refs/review/pr-42-head^{commit}"],
    ]);
  } finally {
    if (previousPath === undefined) delete process.env.PATH;
    else process.env.PATH = previousPath;
    rmSync(directory, { recursive: true, force: true });
  }
});

test("local PR diff: metadata OID 与 fetch 后 ref 漂移时 fail-closed", () => {
  const baseOid = "a".repeat(40);
  const headOid = "b".repeat(40);
  const actualOid = "c".repeat(40);
  for (const [label, expectedOid, ref, refspec] of [
    ["base", baseOid, "refs/review/pr-42-base", "+refs/heads/main:refs/review/pr-42-base"],
    ["head", headOid, "refs/review/pr-42-head", "+refs/pull/42/head:refs/review/pr-42-head"],
  ]) {
    const directory = mkdtempSync(join(tmpdir(), "bong-review-git-drift-"));
    const gitPath = join(directory, "git");
    writeFileSync(
      gitPath,
      `#!/usr/bin/env node
const args = process.argv.slice(2);
const baseOid = ${JSON.stringify(baseOid)};
const headOid = ${JSON.stringify(headOid)};
const label = ${JSON.stringify(label)};
const ref = ${JSON.stringify(ref)};
const refspec = ${JSON.stringify(refspec)};
const actual = ${JSON.stringify(actualOid)};
if (args[0] === "fetch") {
  const expectedRefspec = args[3];
  if (
    expectedRefspec === "+refs/heads/main:refs/review/pr-42-base" ||
    expectedRefspec === "+refs/pull/42/head:refs/review/pr-42-head"
  ) process.exit(0);
  process.exit(2);
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-base^{commit}") {
  process.stdout.write(label === "base" ? actual + "\\n" : baseOid + "\\n");
} else if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-head^{commit}") {
  process.stdout.write(label === "head" ? actual + "\\n" : headOid + "\\n");
} else {
  process.exit(3);
}
`,
    );
    chmodSync(gitPath, 0o755);
    const previousPath = process.env.PATH;
    process.env.PATH = `${directory}:${previousPath || ""}`;
    try {
      assert.throws(
        () => localPrDiff("42", {
          baseRefOid: baseOid,
          baseRefName: "main",
          headRefOid: headOid,
          headRefName: "feature",
        }),
        (error) => error?.message === `PR ${label} OID 漂移：expected=${expectedOid} actual=${actualOid}`,
      );
    } finally {
      if (previousPath === undefined) delete process.env.PATH;
      else process.env.PATH = previousPath;
      rmSync(directory, { recursive: true, force: true });
    }
  }
});

test("local PR diff: fallback 的声明失败均 fail-closed 且保留原因", () => {
  const baseOid = "a".repeat(40);
  const headOid = "b".repeat(40);
  const meta = { baseRefOid: baseOid, baseRefName: "main", headRefOid: headOid };
  const cases = [
    {
      name: "metadata OID 非法",
      meta: { ...meta, baseRefOid: "not-an-oid" },
      expected: "PR metadata 缺少合法 base OID",
    },
    {
      name: "fetch 失败",
      gitScript: `
if (args[0] === "fetch") {
  process.stderr.write("network unavailable\\n");
  process.exit(2);
}
process.exit(3);`,
      expected: `本地 base OID ${baseOid} fetch 失败：network unavailable`,
    },
    {
      name: "fetch 后 ref 不可读",
      gitScript: `
if (args[0] === "fetch") process.exit(0);
if (args[0] === "rev-parse") {
  process.stderr.write("missing ref\\n");
  process.exit(4);
}
process.exit(3);`,
      expected: `本地 base OID ${baseOid} fetch 后 ref 不可读：missing ref`,
    },
    {
      name: "OID 漂移",
      gitScript: `
if (args[0] === "fetch") process.exit(0);
if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-base^{commit}") {
  process.stdout.write(${JSON.stringify("c".repeat(40) + "\n")});
  process.exit(0);
}
process.exit(3);`,
      expected: `PR base OID 漂移：expected=${baseOid} actual=${"c".repeat(40)}`,
    },
    {
      name: "没有 merge-base",
      gitScript: `
if (args[0] === "fetch") process.exit(0);
if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-base^{commit}") {
  process.stdout.write(${JSON.stringify(baseOid + "\n")});
  process.exit(0);
}
if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-head^{commit}") {
  process.stdout.write(${JSON.stringify(headOid + "\n")});
  process.exit(0);
}
if (args[0] === "merge-base") {
  process.stderr.write("no common ancestor\\n");
  process.exit(5);
}
process.exit(3);`,
      expected: "本地 Git 未找到 PR base/head 的 merge-base：no common ancestor",
    },
    {
      name: "local diff 为空",
      gitScript: `
if (args[0] === "fetch") process.exit(0);
if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-base^{commit}") {
  process.stdout.write(${JSON.stringify(baseOid + "\n")});
  process.exit(0);
}
if (args[0] === "rev-parse" && args[2] === "refs/review/pr-42-head^{commit}") {
  process.stdout.write(${JSON.stringify(headOid + "\n")});
  process.exit(0);
}
if (args[0] === "merge-base") {
  process.stdout.write(${JSON.stringify("d".repeat(40) + "\n")});
  process.exit(0);
}
if (args[0] === "diff") process.exit(0);
process.exit(3);`,
      expected: "本地 Git diff 返回空输出",
    },
  ];
  for (const scenario of cases) {
    const scenarioMeta = scenario.meta || meta;
    if (!scenario.gitScript) {
      assert.throws(
        () => localPrDiff("42", scenarioMeta),
        (error) => error?.message === scenario.expected,
        scenario.name,
      );
      continue;
    }
    const directory = mkdtempSync(join(tmpdir(), "bong-review-git-failure-"));
    const gitPath = join(directory, "git");
    writeFileSync(gitPath, `#!/usr/bin/env node
const args = process.argv.slice(2);
${scenario.gitScript}
`);
    chmodSync(gitPath, 0o755);
    const previousPath = process.env.PATH;
    process.env.PATH = `${directory}:${previousPath || ""}`;
    try {
      assert.throws(
        () => localPrDiff("42", scenarioMeta),
        (error) => error?.message === scenario.expected,
        scenario.name,
      );
    } finally {
      if (previousPath === undefined) delete process.env.PATH;
      else process.env.PATH = previousPath;
      rmSync(directory, { recursive: true, force: true });
    }
  }
});

test("loadPrContext: 成功 API diff 的文本不参与 406 判定，只有失败命令才可回退本地", () => {
  const meta = {
    title: "review infra",
    body: "",
    headRefName: "fix/review",
    baseRefName: "main",
    baseRefOid: "a".repeat(40),
    headRefOid: "b".repeat(40),
    files: [{ path: ".github/scripts/review.mjs", additions: 3, deletions: 1 }],
  };
  const error406 = Object.assign(new Error("gh pr diff failed"), {
    code: 1,
    signal: null,
    stdout: "",
    stderr: "HTTP 406: Sorry, the diff exceeded the maximum number of lines (20000)",
  });
  const scenarios = [
    { name: "ordinary API success", apiDiff: "diff --git a/a b/a\n", expectedSource: "api", localCalls: 0 },
    {
      name: "successful diff body mentioning the line limit",
      apiDiff: "diff --git a/a b/a\n+HTTP 406\n+exceeded the maximum number of lines (20000)\n",
      expectedSource: "api",
      localCalls: 0,
    },
    { name: "failed command 406", apiError: error406, expectedSource: "local", localCalls: 1 },
    { name: "ordinary API failure", apiError: new Error("HTTP 502 upstream"), expectedSource: "local", localCalls: 1 },
  ];
  for (const scenario of scenarios) {
    let localCalls = 0;
    const logs = [];
    const context = loadPrContext("42", {
      runGh(args) {
        if (args[1] === "view") return JSON.stringify(meta);
        if (scenario.apiError) throw scenario.apiError;
        return scenario.apiDiff;
      },
      localDiff(pr, receivedMeta) {
        localCalls += 1;
        assert.equal(pr, "42");
        assert.deepEqual(receivedMeta, meta);
        return "diff --git a/local b/local\n";
      },
      log(message) {
        logs.push(message);
      },
    });
    assert.equal(context.diffSource, scenario.expectedSource, scenario.name);
    assert.equal(localCalls, scenario.localCalls, scenario.name);
    assert.deepEqual(logs, [`Review diff source: ${scenario.expectedSource}`]);
  }
});

test("loadPrContext: fallback 的所有声明失败保留原因并合并为 diff acquisition failure", () => {
  const meta = {
    title: "review infra",
    body: "",
    headRefName: "fix/review",
    baseRefName: "main",
    baseRefOid: "a".repeat(40),
    headRefOid: "b".repeat(40),
    files: [],
  };
  const apiError = Object.assign(new Error("gh pr diff failed"), {
    code: 1,
    signal: null,
    stdout: "",
    stderr: "HTTP 406: line limit",
  });
  const localFailures = [
    "PR metadata 缺少合法 base OID",
    `本地 base OID ${meta.baseRefOid} fetch 失败：network unavailable`,
    `本地 head OID ${meta.headRefOid} fetch 后 ref 不可读：missing ref`,
    `PR base OID 漂移：expected=${meta.baseRefOid} actual=${"c".repeat(40)}`,
    "本地 Git 未找到 PR base/head 的 merge-base：no common ancestor",
    "本地 Git diff 失败：git diff returned 128",
    "本地 Git diff 返回空输出",
  ];
  for (const localReason of localFailures) {
    assert.throws(
      () => loadPrContext("42", {
        runGh(args) {
          if (args[1] === "view") return JSON.stringify(meta);
          throw apiError;
        },
        localDiff() {
          throw new Error(localReason);
        },
        log() {
          throw new Error("both paths must fail before logging source");
        },
      }),
      new RegExp(`PR diff acquisition failed \\(api: HTTP 406: line limit; local: ${localReason.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")}\\)`),
      localReason,
    );
  }
});


test("Chat Completions endpoint: 规范化 root/v1/responses/chat/completions 并拒绝凭据与非 HTTPS", () => {
  assert.equal(buildChatCompletionsEndpoint("https://api.example.com"), "https://api.example.com/v1/chat/completions");
  assert.equal(buildChatCompletionsEndpoint("https://api.example.com/v1/"), "https://api.example.com/v1/chat/completions");
  assert.equal(buildChatCompletionsEndpoint("https://api.example.com/v1/responses"), "https://api.example.com/v1/chat/completions");
  assert.equal(
    buildChatCompletionsEndpoint("https://api.example.com/custom/responses/"),
    "https://api.example.com/custom/chat/completions",
  );
  assert.equal(
    buildChatCompletionsEndpoint("https://api.example.com/custom/chat/completions"),
    "https://api.example.com/custom/chat/completions",
  );
  assert.equal(buildChatCompletionsEndpoint("http://127.0.0.1:3000", true), "http://127.0.0.1:3000/v1/chat/completions");
  for (const url of [
    "http://api.example.com",
    "https://user:pass@api.example.com",
    "https://api.example.com?token=x",
    "https://api.example.com/#secret",
  ]) {
    assert.throws(() => buildChatCompletionsEndpoint(url), /HTTPS|凭据/);
  }
});

test("Chat Completions request: SSE、user message、high reasoning，并解析 content", async () => {
  let request;
  const output = '{"vote":"APPROVE"}';
  const chunk = (content, finish_reason = null) => `data: ${JSON.stringify({
    id: "chat-1",
    choices: [{ delta: content === null ? { role: "assistant", content: null } : { content }, finish_reason }],
  })}\n\n`;
  const result = await requestChatCompletions("审查提示", 1_000, {
    apiKey: "provider-secret",
    baseUrl: "https://api.example.com/v1",
    model: "gpt-5.6-sol",
    fetchImpl: async (url, init) => {
      request = { url, init, body: JSON.parse(init.body) };
      return sseResponse([chunk('{"vote":"'), chunk('APPROVE"}'), chunk(null, "stop"), "data: [DONE]\n\n"]);
    },
  });
  assert.equal(result.code, 0);
  assert.equal(result.stdout, output);
  assert.equal(request.url, "https://api.example.com/v1/chat/completions");
  assert.equal(request.init.headers.Authorization, "Bearer provider-secret");
  assert.equal(request.init.headers.Accept, "text/event-stream");
  assert.deepEqual(request.body, {
    model: "gpt-5.6-sol",
    messages: [{ role: "user", content: "审查提示" }],
    reasoning_effort: "high",
    store: false,
    stream: true,
  });
  assert.equal(Object.hasOwn(request.body, "tools"), false, "生产 reviewer 不得获得 shell 或其他 tool");
  assert.equal(extractChatCompletionsOutputText({ choices: [{ message: { content: "chat output" } }] }), "chat output");
  assert.equal(extractChatCompletionsOutputText({ choices: [{ message: { content: "second output" } }] }), "second output");
});

test("Chat Completions SSE: content:null 是无文本首帧，随后 content 与 DONE 可成功", async () => {
  const result = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse([chatSseChunk(null), chatSseChunk("content", "stop"), chatSseDone()]),
  });
  assert.equal(result.code, 0);
  assert.equal(result.stdout, "content");
});

test("Chat Completions SSE: transport compatibility and semantic transition table", async () => {
  const completion = (content, finishReason = null) => JSON.stringify({
    choices: [{ delta: { content }, finish_reason: finishReason }],
  });
  const rawFrame = (lines) => `${lines.join("\n")}\n\n`;
  const frame = ({ event, data, extraFields = [] }) => {
    const lines = [];
    if (event !== undefined) lines.push(`event: ${event}`);
    for (const [field, value] of extraFields) lines.push(value === undefined ? field : `${field}: ${value}`);
    if (data !== undefined) {
      for (const line of Array.isArray(data) ? data : [data]) lines.push(`data: ${line}`);
    }
    return rawFrame(lines);
  };
  const stopped = (content) => frame({ data: completion(content, "stop") });
  const usage = frame({
    data: JSON.stringify({ choices: [], usage: { total_tokens: 1 }, service_tier: "priority" }),
  });
  const done = chatSseDone();
  const cases = [
    {
      name: "unnamed data frame 的 choice → stop → DONE",
      stream: [stopped("unnamed"), done],
      output: "unnamed",
    },
    {
      name: "event: message 的 choice → stop → DONE",
      stream: [
        frame({ event: "message", data: completion("message", "stop") }),
        frame({ event: "message", data: "[DONE]" }),
      ],
      output: "message",
    },
    {
      name: "多个 data field 按 SSE 规则拼接为一个 completion",
      stream: [
        frame({
          data: [
            '{"choices":[{"delta":{"content":"split"},',
            '"finish_reason":"stop"}]}',
          ],
        }),
        done,
      ],
      output: "split",
    },
    {
      name: "标准 SSE 注释、metadata、扩展字段、重复 event 与任意字段顺序均兼容",
      stream: [
        rawFrame([
          ": keepalive",
          `data: ${completion("compatible", "stop")}`,
          "id: 42",
          "event: ignored-first",
          "retry: 1000",
          "x-provider-extension: future",
          "event: message",
          ": still ignored",
        ]),
        rawFrame([
          "id: 43",
          "event: ignored-first",
          "data: [DONE]",
          "retry: 2000",
          "event: message",
        ]),
      ],
      output: "compatible",
    },
    {
      name: "stop 后允许含 service_tier 的一次 usage-only 再 DONE",
      stream: [stopped("usage"), usage, done],
      output: "usage",
    },
    {
      name: "最后一个 event: error 即使承载完整 stop payload 和 DONE 也作为 provider failure",
      stream: [
        rawFrame([
          `data: ${completion("laundered", "stop")}`,
          "event: message",
          "event: error",
        ]),
        done,
      ],
      error: /provider error event/,
      stdout: "",
    },
    {
      name: "usage-only 后 event: error 仍作为 provider failure",
      stream: [stopped("partial"), usage, frame({ event: "error", data: "not-json" })],
      error: /provider error event/,
      stdout: "partial",
    },
    {
      name: "bare event: error 即使后续 completion 与 DONE 完整也 fail-closed",
      stream: [frame({ event: "error" }), stopped("laundered"), done],
      error: /provider error frame 未携带 detail/,
      stdout: "",
    },
    {
      name: "无 data 的 event: message 作为 transport metadata 忽略",
      stream: [frame({ event: "message" }), stopped("metadata"), done],
      output: "metadata",
    },
    {
      name: "最后一个未知 event 名即使承载完整 stop payload 和 DONE 也拒绝",
      stream: [
        rawFrame([
          "event: message",
          `data: ${completion("laundered", "stop")}`,
          "event: chunk",
        ]),
        done,
      ],
      error: /不支持的 SSE event 名：chunk/,
      stdout: "",
    },
    {
      name: "未知 SSE field 作为 transport metadata 忽略",
      stream: [frame({ data: completion("metadata", "stop"), extraFields: [["id", "7"]] }), done],
      output: "metadata",
    },
    {
      name: "一个 frame 允许重复 event field 且最后一个值生效",
      stream: [
        frame({
          event: "ignored-first",
          data: completion("last-event", "stop"),
          extraFields: [["event", "message"]],
        }),
        done,
      ],
      output: "last-event",
    },
    {
      name: "空 data field 的 payload 语义拒绝",
      stream: [frame({ data: "" }), done],
      error: /SSE frame data 不得为空/,
    },
    {
      name: "open 阶段的 DONE 拒绝",
      stream: [done],
      error: /缺少 finish_reason=stop/,
    },
    {
      name: "stop 后普通 choice 拒绝",
      stream: [stopped("partial"), frame({ data: completion("tail") }), done],
      error: /finish_reason 后不得再出现普通 choice event/,
      stdout: "partial",
    },
    {
      name: "usage-only 后普通 choice 拒绝",
      stream: [stopped("partial"), usage, frame({ data: completion("tail") }), done],
      error: /usage-only event 后只允许 \[DONE\]/,
      stdout: "partial",
    },
    {
      name: "usage-only 后第二个 usage-only 拒绝",
      stream: [stopped("partial"), usage, usage, done],
      error: /usage-only event 后只允许 \[DONE\]/,
      stdout: "partial",
    },
    {
      name: "stop 后的 comment 作为传输 keepalive 忽略",
      stream: [stopped("partial"), ": late\n\n", done],
      output: "partial",
    },
    {
      name: "DONE 立即终止并忽略同一缓冲区后续帧",
      stream: [
        stopped("complete"),
        done,
        frame({ event: "chunk", data: "not-json" }),
        frame({ event: "error", data: "not-json" }),
      ],
      output: "complete",
    },
  ];

  for (const scenario of cases) {
    const result = await requestChatCompletions("prompt", 1_000, {
      apiKey: "key",
      baseUrl: "https://api.example.com",
      fetchImpl: async () => sseResponse(scenario.stream),
    });
    if (scenario.error) {
      assert.equal(result.code, 1, scenario.name);
      assert.equal(isSuccessfulCodexResponse(result), false, scenario.name);
      assert.match(result.stderr, scenario.error, scenario.name);
      if (scenario.stdout !== undefined) assert.equal(result.stdout, scenario.stdout, scenario.name);
    } else {
      assert.equal(result.code, 0, scenario.name);
      assert.equal(result.stdout, scenario.output, scenario.name);
    }
  }
});

test("Chat Completions SSE: DONE 立即完成并取消保持开启的 reader", async () => {
  let cancelled = false;
  const response = sseResponse([chatSseChunk("complete", "stop"), chatSseDone()], {
    neverClose: true,
    cancel: () => {
      cancelled = true;
    },
  });
  const result = await Promise.race([
    requestChatCompletions("prompt", 1_000, {
      apiKey: "key",
      baseUrl: "https://api.example.com",
      fetchImpl: async () => response,
    }),
    new Promise((_, reject) => setTimeout(() => reject(new Error("[DONE] 后仍在等待 EOF")), 100)),
  ]);
  assert.equal(result.code, 0);
  assert.equal(result.stdout, "complete");
  assert.equal(cancelled, true);
});

test("Chat Completions SSE: stop 和 DONE 后仍无 content 时 fail-closed", async () => {
  const result = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse([chatSseChunk(null), chatSseChunk("", "stop"), chatSseDone()]),
  });
  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.match(result.stderr, /Chat Completions 未返回 content。/);
});

test("Chat Completions SSE: choices/choice/delta 结构损坏时，即使随后 DONE 也 fail-closed", async () => {
  const malformedEvents = [
    JSON.stringify({ id: "missing-choices" }),
    JSON.stringify({ choices: "not-an-array" }),
    JSON.stringify({ choices: ["not-an-object"] }),
    JSON.stringify({ choices: [{}] }),
    JSON.stringify({ choices: [{ delta: "not-an-object" }] }),
    JSON.stringify({ choices: [{ delta: { content: 42 } }] }),
  ];
  for (const data of malformedEvents) {
    const result = await requestChatCompletions("prompt", 1_000, {
      apiKey: "key",
      baseUrl: "https://api.example.com",
      fetchImpl: async () => sseResponse([chatSseChunk("partial"), `data: ${data}\n\n`, chatSseDone()]),
    });
    assert.equal(result.code, 1, `malformed event 不得成功：${data}`);
    assert.equal(result.stdout, "partial");
    assert.match(result.stderr, /Chat Completions SSE/);
  }
});

test("Chat Completions SSE: usage-only 仅允许 stop 后一次且随后必须 DONE", async () => {
  const usageOnly = `data: ${JSON.stringify({ choices: [], usage: { total_tokens: 1 } })}\n\n`;
  const cases = [
    {
      name: "content 前 usage-only",
      stream: [usageOnly, chatSseChunk("content", "stop"), chatSseDone()],
      error: /usage-only event 必须出现在 finish_reason=stop 之后/,
    },
    {
      name: "stop 后一次 usage-only 再 DONE",
      stream: [chatSseChunk("content", "stop"), usageOnly, chatSseDone()],
      output: "content",
    },
    {
      name: "usage-only 后普通 choice",
      stream: [chatSseChunk("content", "stop"), usageOnly, chatSseChunk("tail"), chatSseDone()],
      error: /usage-only event 后只允许 \[DONE\]/,
    },
    {
      name: "usage-only 后第二个 usage-only",
      stream: [chatSseChunk("content", "stop"), usageOnly, usageOnly, chatSseDone()],
      error: /usage-only event 后只允许 \[DONE\]/,
    },
  ];
  for (const scenario of cases) {
    const result = await requestChatCompletions("prompt", 1_000, {
      apiKey: "key",
      baseUrl: "https://api.example.com",
      fetchImpl: async () => sseResponse(scenario.stream),
    });
    if (scenario.error) {
      assert.equal(result.code, 1, scenario.name);
      assert.match(result.stderr, scenario.error, scenario.name);
    } else {
      assert.equal(result.code, 0, scenario.name);
      assert.equal(result.stdout, scenario.output, scenario.name);
    }
  }
});

test("Chat Completions SSE: choices 空数组拒绝非 usage-only 形态", async () => {
  const malformed = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse(["data: {\"choices\":[]}\n\n", chatSseDone()]),
  });
  assert.equal(malformed.code, 1);
  assert.match(malformed.stderr, /choices 为空/);
});

test("Chat Completions SSE: finish_reason 只接受 stop，stop 后仅可 usage-only 再 DONE", async () => {
  const stopped = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse([
      chatSseChunk("complete", "stop"),
      `data: ${JSON.stringify({ choices: [], usage: { total_tokens: 1 } })}\n\n`,
      chatSseDone(),
    ]),
  });
  assert.equal(stopped.code, 0);
  assert.equal(stopped.stdout, "complete");

  for (const finishReason of ["length", "content_filter", "unknown"]) {
    const result = await requestChatCompletions("prompt", 1_000, {
      apiKey: "key",
      baseUrl: "https://api.example.com",
      fetchImpl: async () => sseResponse([chatSseChunk("partial", finishReason), chatSseDone()]),
    });
    assert.equal(result.code, 1, `${finishReason} 不得成功`);
    assert.equal(result.stdout, "partial");
    assert.match(result.stderr, new RegExp(`finish_reason=${finishReason}`));
  }

  const afterStop = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse([chatSseChunk("partial", "stop"), chatSseChunk("tail"), chatSseDone()]),
  });
  assert.equal(afterStop.code, 1);
  assert.equal(afterStop.stdout, "partial");
  assert.match(afterStop.stderr, /finish_reason 后不得再出现普通 choice event/);

  const noFinishReason = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse([chatSseChunk("partial"), chatSseDone()]),
  });
  assert.equal(noFinishReason.code, 1);
  assert.equal(noFinishReason.stdout, "partial");
  assert.match(noFinishReason.stderr, /缺少 finish_reason=stop/);
});

test("Chat Completions request: HTTP 错误保留 final text，malformed 与 timeout 可分类", async () => {
  const finalOn503 = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => ({
      ok: false,
      status: 503,
      statusText: "Service Unavailable",
      text: async () => JSON.stringify({
        error: { message: "upstream unavailable" },
        output: [{ type: "message", content: [{ type: "output_text", text: '{"confidence":0,"findings":[]}' }] }],
      }),
    }),
  });
  assert.equal(finalOn503.code, 503);
  assert.equal(finalOn503.stdout, '{"confidence":0,"findings":[]}');
  assert.match(finalOn503.stderr, /HTTP 503/);
  assert.equal(isSuccessfulCodexResponse(finalOn503), false);

  for (const scenario of [
    {
      status: 503,
      statusText: "Service Unavailable",
      output: { vote: "APPROVE", confidence: 95, findings: [] },
    },
    {
      status: 429,
      statusText: "Too Many Requests",
      output: {
        vote: "REQUEST_CHANGES",
        confidence: 95,
        findings: [{ file: "server/src/main.rs", title: "真实代码缺陷" }],
      },
    },
  ]) {
    const failed = await requestChatCompletions("prompt", 1_000, {
      apiKey: "key",
      baseUrl: "https://api.example.com",
      fetchImpl: async () => ({
        ok: false,
        status: scenario.status,
        statusText: scenario.statusText,
        text: async () => JSON.stringify({
          error: { message: scenario.statusText },
          output: [{ type: "message", content: [{ type: "output_text", text: JSON.stringify(scenario.output) }] }],
        }),
      }),
    });
    assert.equal(failed.code, scenario.status);
    assert.deepEqual(JSON.parse(failed.stdout), scenario.output);
    assert.equal(isSuccessfulCodexResponse(failed), false, `HTTP ${scenario.status} 的 final text 不得伪装成功`);
  }

  assert.equal(
    isSuccessfulCodexResponse({ code: 0, stdout: '{"vote":"APPROVE","confidence":95,"findings":[]}' }),
    true,
  );

  const malformed = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse(["data: not-json\n\n"]),
  });
  assert.equal(malformed.code, 1);
  assert.match(malformed.stderr, /不是合法 JSON/);

  const timedOut = await requestChatCompletions("prompt", 10, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async (_url, init) => new Promise((_resolve, reject) => {
      init.signal.addEventListener("abort", () => {
        const error = new Error("aborted");
        error.name = "AbortError";
        reject(error);
      });
    }),
  });
  assert.equal(timedOut.code, 124);
  assert.equal(timedOut.signal, "SIGTERM");
});


test("Chat Completions SSE: UTF-8 分片、CRLF/comment 与 content", async () => {
  const chunk = `data: ${JSON.stringify({ choices: [{ delta: { content: "中文🙂" }, finish_reason: null }] })}\r\n\r\n`;
  const end = `data: ${JSON.stringify({ choices: [{ delta: {}, finish_reason: "stop" }] })}\r\n\r\n`;
  const done = "data: [DONE]\r\n\r\n";
  const bytes = textEncoder.encode(`: ping\r\n\r\n${chunk}${end}${done}`);
  const cut = bytes.indexOf(0x9f);
  assert.ok(cut > 0, "fixture 必须把 emoji 的 UTF-8 字节拆到两个网络 chunk");

  const result = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse([bytes.slice(0, cut), bytes.slice(cut)]),
  });
  assert.equal(result.code, 0);
  assert.equal(result.stdout, "中文🙂");
});

test("Chat Completions SSE: provider error、断流与 malformed 均不得假成功", async () => {
  const chunk = (content, finish_reason = null) => `data: ${JSON.stringify({
    choices: [{ delta: content === null ? { role: "assistant", content: null } : { content }, finish_reason }],
  })}\n\n`;
  const cases = [
    { stream: [`data: ${JSON.stringify({ error: { message: "provider exploded" } })}\n\n`], error: /provider exploded/ },
    { stream: [chatSseChunk("partial")], error: /disconnected/ },
    { stream: ["data: not-json\n\n"], error: /不是合法 JSON/ },
  ];
  for (const scenario of cases) {
    const result = await requestChatCompletions("prompt", 1_000, {
      apiKey: "key",
      baseUrl: "https://api.example.com",
      fetchImpl: async () => sseResponse(scenario.stream),
    });
    assert.equal(result.code, 1);
    assert.equal(isSuccessfulCodexResponse(result), false);
    assert.match(result.stderr, scenario.error);
  }
  const doneOnly = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse(["data: [DONE]\n\n"]),
  });
  assert.equal(doneOnly.code, 1, "空 content 不得以 code 0 伪装成 genuine completion");
  assert.equal(doneOnly.stdout, "");
  assert.match(doneOnly.stderr, /缺少 finish_reason=stop/);

  const disconnected = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => sseResponse([`data: ${JSON.stringify({ choices: [{ delta: {} }] })}`]),
  });
  assert.equal(disconnected.code, 1);
  assert.match(disconnected.stderr, /disconnected/);
});

test("Chat Completions SSE: reader error 与绝对 timeout 保留 partial stdout", async () => {
  const partialFrame = `data: ${JSON.stringify({ choices: [{ delta: { content: "partial" }, finish_reason: null }] })}\n\n`;
  const readerFailed = await requestChatCompletions("prompt", 1_000, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async () => ({
      ok: true,
      status: 200,
      statusText: "OK",
      body: new ReadableStream({
        pull(controller) {
          if (!this.sentPartial) {
            this.sentPartial = true;
            controller.enqueue(textEncoder.encode(partialFrame));
            return;
          }
          controller.error(new Error("reader boom"));
        },
      }),
    }),
  });
  assert.equal(readerFailed.code, 1);
  assert.equal(readerFailed.stdout, "partial");
  assert.match(readerFailed.stderr, /reader boom/);

  const timedOut = await requestChatCompletions("prompt", 20, {
    apiKey: "key",
    baseUrl: "https://api.example.com",
    fetchImpl: async (_url, init) => ({
      ok: true,
      status: 200,
      statusText: "OK",
      body: new ReadableStream({
        start(controller) {
          controller.enqueue(textEncoder.encode(partialFrame));
          const onAbort = () => controller.error(Object.assign(new Error("aborted"), { name: "AbortError" }));
          init.signal.addEventListener("abort", onAbort, { once: true });
        },
      }),
    }),
  });
  assert.equal(timedOut.code, 124);
  assert.equal(timedOut.signal, "SIGTERM");
  assert.equal(timedOut.stdout, "partial");
});
test("evaluateCircuit: 阈值前关闭，达到阈值后开启", () => {
  const events = ["00:00", "00:10", "00:20"].map((time) => ({
    kind: "infra_failure",
    at: `2026-07-10T${time}:00.000Z`,
  }));
  assert.equal(evaluateCircuit(events.slice(0, 2), "2026-07-10T00:20:00.000Z").open, false);
  const state = evaluateCircuit(events, "2026-07-10T00:20:00.000Z");
  assert.equal(state.open, true);
  assert.equal(state.openUntil, "2026-07-10T01:20:00.000Z");
});

test("evaluateCircuit: 熔断截止时刻精确过期，窗口边界计入", () => {
  const events = ["00:00", "00:30", "01:00"].map((time) => ({
    kind: "infra_failure",
    at: `2026-07-10T${time}:00.000Z`,
  }));
  assert.equal(evaluateCircuit(events, "2026-07-10T01:59:59.999Z").open, true);
  assert.equal(evaluateCircuit(events, "2026-07-10T02:00:00.000Z").open, false);
});

test("evaluateCircuit: 窗口外 1ms、未来事件、threshold=1 与后续失败延长", () => {
  const base = [
    { kind: "infra_failure", at: "2026-07-10T00:00:00.000Z" },
    { kind: "infra_failure", at: "2026-07-10T00:30:00.000Z" },
    { kind: "infra_failure", at: "2026-07-10T01:00:00.001Z" },
  ];
  assert.equal(evaluateCircuit(base, "2026-07-10T01:00:00.001Z").open, false, "超过闭区间 1ms 不得开闸");
  assert.equal(
    evaluateCircuit([{ kind: "infra_failure", at: "2026-07-10T00:00:00.000Z" }], "2026-07-10T00:00:00.000Z", {
      threshold: 1,
      windowMs: 60_000,
      durationMs: 60_000,
    }).open,
    true,
  );
  assert.equal(
    evaluateCircuit(
      ["00:00", "00:10", "00:21"].map((time) => ({
        kind: "infra_failure",
        at: `2026-07-10T${time}:00.000Z`,
      })),
      "2026-07-10T00:20:00.000Z",
    ).failureCount,
    2,
    "未来事件不得进入当前失败计数",
  );
  const extended = evaluateCircuit(
    ["00:00", "00:10", "00:20", "00:30"].map((time) => ({
      kind: "infra_failure",
      at: `2026-07-10T${time}:00.000Z`,
    })),
    "2026-07-10T00:30:00.000Z",
  );
  assert.equal(extended.openUntil, "2026-07-10T01:30:00.000Z");
});

test("manual bypass: 熔断期间仅 issue_comment 的精确 /review 入口可旁路", () => {
  assert.equal(isCircuitBypassTrigger("issue_comment", "/review"), true);
  for (const [eventName, body] of [
    ["pull_request", "/review"],
    ["workflow_dispatch", "/review"],
    ["issue_comment", ""],
    ["issue_comment", "/review now"],
    ["issue_comment", " /review"],
    ["issue_comment", "/review\n"],
  ]) {
    assert.equal(isCircuitBypassTrigger(eventName, body), false, `${eventName}:${JSON.stringify(body)} 不得旁路`);
  }
});

test("circuit-preflight 命令: 熔断中仅精确 /review 旁路，自动与兜底触发均暂停", () => {
  const common = {
    REVIEW_NOW: "2026-07-10T00:20:00.000Z",
    REVIEW_INFRA_FAILURE_THRESHOLD: "3",
    REVIEW_INFRA_FAILURE_WINDOW_MINUTES: "60",
    REVIEW_CIRCUIT_MINUTES: "60",
  };
  const manual = runReviewCommand("circuit-preflight", {
    env: { ...common, REVIEW_TRIGGER: "issue_comment", REVIEW_COMMENT_BODY: "/review" },
  });
  assert.equal(manual.status, 0, manual.stderr);
  assert.equal(manual.githubOutput, "should_run=true\n");

  for (const [trigger, body] of [
    ["pull_request", ""],
    ["workflow_dispatch", ""],
    ["issue_comment", "/review now"],
  ]) {
    const paused = runReviewCommand("circuit-preflight", {
      env: { ...common, REVIEW_TRIGGER: trigger, REVIEW_COMMENT_BODY: body },
      ghScript: openCircuitGhScript(),
    });
    assert.equal(paused.status, 0, paused.stderr);
    assert.equal(paused.githubOutput, "should_run=false\n", `${trigger}:${body} 应在熔断期暂停`);
    assert.match(paused.stderr, /熔断跳过/);
    assert.doesNotMatch(paused.stderr, /发布 Review 熔断跳过评论失败/);
  }
});

test("circuit-preflight 命令: 未熔断继续，Search 失败 fail-open，跳过评论失败仍成功", () => {
  const closed = runReviewCommand("circuit-preflight", {
    env: { REVIEW_TRIGGER: "pull_request_target" },
    ghScript: recordingGhScript(),
  });
  assert.equal(closed.status, 0, closed.stderr);
  assert.equal(closed.githubOutput, "should_run=true\n");

  const failOpen = runReviewCommand("circuit-preflight", {
    env: { REVIEW_TRIGGER: "pull_request_target" },
    ghScript: `
const args = process.argv.slice(2).join(" ");
if (args.includes("search/issues")) process.exit(1);
process.exit(92);
`,
  });
  assert.equal(failOpen.status, 0, failOpen.stderr);
  assert.equal(failOpen.githubOutput, "should_run=true\n");
  assert.match(failOpen.stderr, /fail-open/);

  const commentFailureScript = openCircuitGhScript().replace(
    'else if (args.includes("issues/1148/comments")) process.stdout.write("{}");',
    'else if (args.includes("issues/1148/comments")) process.exit(1);',
  );
  const skipped = runReviewCommand("circuit-preflight", {
    env: { REVIEW_TRIGGER: "pull_request_target" },
    ghScript: commentFailureScript,
  });
  assert.equal(skipped.status, 0, skipped.stderr);
  assert.equal(skipped.githubOutput, "should_run=false\n");
  assert.equal(skipped.reviewOutcome.kind, "circuit_skipped");
  assert.match(skipped.stderr, /发布 Review 熔断跳过评论失败/);
});

test("infra-only classification: 仅四路纯执行失败降级，混合结果保留真实 REQUEST_CHANGES", () => {
  const allInfra = ["A", "B", "C", "D"].map((id) => ({
    ...infraResult,
    reviewer: id,
    findings: [{ file: ".github/scripts/review.mjs", title: `Codex reviewer initial-${id} 执行失败` }],
  }));
  assert.equal(classifyReviewRun(allInfra, allInfra), "infra_failure");
  assert.equal(
    classifyReviewRun(allInfra, [allInfra[0], realRequestChanges, realRequestChanges, realRequestChanges]),
    "gate_failure",
    "单路 503 不得吞掉其他 reviewer 的真实代码 finding",
  );
  assert.equal(
    classifyReviewRun([realRequestChanges, ...allInfra.slice(1)], allInfra),
    "gate_failure",
    "首轮已有真实代码 finding 时，复投全 503 也不得整体降级忽略",
  );
  assert.equal(
    classifyReviewRun(allInfra, [{ ...allInfra[0], findings: [...allInfra[0].findings, realRequestChanges.findings[0]] }, ...allInfra.slice(1)]),
    "gate_failure",
    "confidence 0 结果混入真实代码 finding 时不得伪装成纯基础设施失败",
  );
  assert.equal(classifyReviewRun([realRequestChanges], [realRequestChanges]), "gate_failure");
  const approve = { vote: "APPROVE", confidence: 90, findings: [] };
  assert.equal(classifyReviewRun([approve], [approve, approve, approve, realRequestChanges]), "gate_failure");
  assert.equal(decideReviewGate([approve, approve, approve, realRequestChanges]).passed, false);
  assert.match(
    decideReviewGate([approve, approve, approve, realRequestChanges], [realRequestChanges, realRequestChanges]).label,
    /存在 1 项真实 finding/,
  );
  assert.equal(decideReviewGate([approve, approve, approve, { ...realRequestChanges, findings: [] }]).passed, true);
  const spoofedSynthetic = {
    vote: "REQUEST_CHANGES",
    confidence: 0,
    execution_failure: false,
    findings: [{ file: ".github/scripts/review.mjs", title: "Codex reviewer final-D 执行失败" }],
  };
  assert.equal(decideReviewGate([approve, approve, approve, spoofedSynthetic]).passed, false);
  assert.equal(decideReviewGate([approve, approve, approve, { ...spoofedSynthetic, execution_failure: true }]).passed, true);

  const completeApprove = Array(4).fill(approve);
  assert.equal(
    classifyReviewRun([infraResult, approve, approve, approve], completeApprove),
    "infra_failure",
    "首轮任一路执行失败不得被复投四票 APPROVE 覆盖",
  );
  assert.equal(
    classifyReviewRun([approve, approve, approve], completeApprove),
    "infra_failure",
    "首轮缺票时不得仅凭完整复投通过",
  );
  assert.equal(
    classifyReviewRun(completeApprove, [approve, approve, approve, { ...approve, execution_failure: true }]),
    "infra_failure",
    "HTTP 错误携带高置信 APPROVE 时仍属于执行失败",
  );
  assert.equal(
    classifyReviewRun(
      completeApprove,
      [approve, approve, approve, { ...realRequestChanges, execution_failure: true }],
    ),
    "gate_failure",
    "HTTP 错误携带真实 finding 时必须优先阻断",
  );
  assert.equal(
    classifyReviewRun(completeApprove, [approve, approve, approve, infraResult]),
    "infra_failure",
    "复投任一路执行失败时面板不完整，不得靠 3/1 通过",
  );
  assert.equal(
    classifyReviewRun([realRequestChanges, approve, approve, approve], completeApprove),
    "gate_failure",
    "首轮真实 finding 即使复投撤回也必须阻塞",
  );
  assert.equal(
    classifyReviewRun([realRequestChanges, approve, approve, approve], [approve, approve, approve, infraResult]),
    "gate_failure",
    "真实 finding 与执行失败并存时，真实代码问题优先于 infra handoff",
  );
});

test("infra-only classification: 模型给出真实审查或不可解析内容都不伪造 infra", () => {
  const malformed = normalizeResult("不是 JSON", reviewer);
  assert.equal(classifyReviewRun([malformed], [malformed]), "gate_failure");
  const spoofed = normalizeResult(
    JSON.stringify({
      execution_failure: true,
      vote: "REQUEST_CHANGES",
      confidence: 0,
      findings: [{ file: ".github/scripts/review.mjs", title: "Codex reviewer initial-A 执行失败" }],
    }),
    reviewer,
  );
  assert.equal(spoofed.execution_failure, false, "模型 JSON 不得伪造进程层 execution_failure 标记");
  assert.equal(classifyReviewRun(Array(4).fill(spoofed), Array(4).fill(spoofed)), "gate_failure");
});

test("provider final message: 仅 confidence 0 且无代码 finding 识别为 infra", () => {
  const emptyFailureRaw = JSON.stringify({
    vote: "REQUEST_CHANGES",
    confidence: 0,
    plan_intent: { status: "unclear", reason: "503", missing: [] },
    summary: "provider 503",
    findings: [],
  });
  assert.equal(isZeroConfidenceWithoutCodeFindings(emptyFailureRaw), true);
  const emptyFailure = normalizeResult(emptyFailureRaw, reviewer, { executionFailure: true });
  assert.equal(classifyReviewRun(Array(4).fill(emptyFailure), Array(4).fill(emptyFailure)), "infra_failure");

  const codeFindingRaw = JSON.stringify({
    vote: "REQUEST_CHANGES",
    confidence: 0,
    plan_intent: { status: "not_plan", reason: "发现代码问题", missing: [] },
    summary: "真实问题",
    findings: [{ severity: "major", file: "server/src/main.rs", line: "1", title: "真实代码缺陷" }],
  });
  assert.equal(isZeroConfidenceWithoutCodeFindings(codeFindingRaw), false);
  const codeFinding = normalizeResult(codeFindingRaw, reviewer, { executionFailure: true });
  assert.equal(classifyReviewRun(Array(4).fill(codeFinding), Array(4).fill(codeFinding)), "gate_failure");
  for (const malformed of ["", "not-json", JSON.stringify({ confidence: 0 }), JSON.stringify({ confidence: 1, findings: [] })]) {
    assert.equal(Boolean(isZeroConfidenceWithoutCodeFindings(malformed)), false);
  }
});

test("mixed review findings: 复投执行失败时保留同路首轮代码 finding", () => {
  const firstRound = [realRequestChanges, infraResult, realRequestChanges, infraResult];
  const finalRound = [infraResult, infraResult, realRequestChanges, infraResult];
  const retained = reviewFindingResults(firstRound, finalRound);
  assert.deepEqual(retained.slice(0, 4), firstRound);
  assert.deepEqual(retained.slice(4), finalRound);
  assert.equal(retained.length, 8);
});


test("workflow finalize: 真实 gate failure 原样失败，setup/reviewer 异常才记录 infra", () => {
  for (const review of ["success", "failure"]) {
    assert.deepEqual(
      classifyWorkflowFinalization({ install: "success", configure: "success", review, outcomeKind: "gate_failure" }),
      { kind: "gate_failure", shouldRecord: false },
      "真实 REQUEST_CHANGES 不得因 review step 是否返回非零而污染 infra 熔断计数",
    );
  }
  for (const outcome of ["failure", "skipped", "cancelled"]) {
    assert.equal(classifyWorkflowFinalization({ circuit: outcome }).phase, "circuit_preflight");
    assert.equal(classifyWorkflowFinalization({ reviewJob: outcome }).phase, "review_job");
    assert.equal(classifyWorkflowFinalization({ install: outcome }).phase, "cli_install");
    assert.equal(classifyWorkflowFinalization({ install: "success", configure: outcome }).phase, "provider_config");
    assert.equal(
      classifyWorkflowFinalization({ install: "success", configure: "success", checkout: outcome }).phase,
      "pr_checkout",
    );
  }
  assert.equal(classifyWorkflowFinalization({ install: "success", configure: "success", review: "failure" }).phase, "review_process");
  assert.equal(
    classifyWorkflowFinalization({ install: "success", configure: "success", review: "failure", outcomeKind: "infra_failure" }).shouldRecord,
    false,
  );
  assert.deepEqual(
    classifyWorkflowFinalization({ install: "success", configure: "success", review: "success", outcomeKind: "infra_failure" }),
    { kind: "infra_failure", shouldRecord: false },
  );
  for (const outcomeKind of [null, "unknown"]) {
    assert.equal(
      classifyWorkflowFinalization({ install: "success", configure: "success", review: "success", outcomeKind }).phase,
      "review_process",
    );
  }
  assert.deepEqual(
    classifyWorkflowFinalization({ install: "success", configure: "success", review: "success", outcomeKind: "passed" }),
    { kind: "passed", shouldRecord: false },
  );
});

test("workflow-finalize 命令: step success 的真实 REQUEST_CHANGES 仍失败且不写 infra", () => {
  const result = runReviewCommand("workflow-finalize", {
    env: {
      REVIEW_INSTALL_OUTCOME: "success",
      REVIEW_CONFIGURE_OUTCOME: "success",
      REVIEW_STEP_OUTCOME: "success",
    },
    outcome: { kind: "gate_failure", gate: "REQUEST_CHANGES" },
  });
  assert.equal(result.status, 1, result.stderr);
  assert.doesNotMatch(result.stderr, /持久化 Review infra failure/);
});

test("workflow-finalize 命令: setup/预检/checkout 失败持久化 marker 并发布 handoff 后成功退出", () => {
  const cases = [
    { env: { REVIEW_CIRCUIT_OUTCOME: "failure" }, phase: "circuit_preflight" },
    { env: { REVIEW_INSTALL_OUTCOME: "failure" }, phase: "cli_install" },
    {
      env: { REVIEW_INSTALL_OUTCOME: "success", REVIEW_CONFIGURE_OUTCOME: "failure" },
      phase: "provider_config",
    },
    {
      env: {
        REVIEW_INSTALL_OUTCOME: "success",
        REVIEW_CONFIGURE_OUTCOME: "success",
        REVIEW_CHECKOUT_OUTCOME: "failure",
      },
      phase: "pr_checkout",
    },
    {
      env: {
        REVIEW_INSTALL_OUTCOME: "success",
        REVIEW_CONFIGURE_OUTCOME: "success",
        REVIEW_CHECKOUT_OUTCOME: "success",
        REVIEW_STEP_OUTCOME: "failure",
      },
      phase: "review_process",
    },
  ];

  for (const row of cases) {
    const result = runReviewCommand("workflow-finalize", {
      env: row.env,
      ghScript: recordingGhScript(),
    });
    assert.equal(result.status, 0, `${row.phase}: ${result.stderr}`);
    assert.equal(result.reviewOutcome.kind, "infra_failure");
    const log = parseGitHubJsonLines(result.ghLog);
    const stateComment = log.find((entry) => entry.kind === "state_comment");
    const handoff = log.find((entry) => entry.kind === "handoff_comment");
    assert.match(stateComment?.body || "", /bong-review-circuit/);
    assert.match(stateComment?.body || "", new RegExp(row.phase));
    assert.match(handoff?.body || "", /请忽略本次 Review Action 结果/);
    assert.match(handoff?.body || "", /改走 agent 自有博弈式 review 流程并向用户反馈/);
  }
});

test("workflow-finalize 命令: 同 run 去重；状态持久化或 handoff 评论失败均不中断", () => {
  const existing = {
    body: renderHiddenMarker("bong-review-circuit", {
      v: 1,
      kind: "infra_failure",
      run_id: "9001",
      at: "2026-07-10T00:10:00.000Z",
    }),
    user: { login: "github-actions[bot]", type: "Bot" },
  };
  const deduped = runReviewCommand("workflow-finalize", {
    env: { REVIEW_INSTALL_OUTCOME: "failure" },
    ghScript: recordingGhScript([existing]),
  });
  assert.equal(deduped.status, 0, deduped.stderr);
  const dedupeLog = parseGitHubJsonLines(deduped.ghLog);
  assert.equal(dedupeLog.filter((entry) => entry.kind === "state_comment").length, 0);
  assert.equal(dedupeLog.filter((entry) => entry.kind === "handoff_comment").length, 1);

  const persistenceFailure = runReviewCommand("workflow-finalize", {
    env: { REVIEW_INSTALL_OUTCOME: "failure" },
    ghScript: `
const fs = require("node:fs");
const args = process.argv.slice(2);
const joined = args.join(" ");
fs.appendFileSync(process.env.GH_TEST_LOG, JSON.stringify({ args }) + "\\n");
if (joined.includes("search/issues")) process.exit(1);
if (joined.includes("issues/1148/comments")) {
  fs.appendFileSync(process.env.GH_TEST_LOG, JSON.stringify({ kind: "handoff_after_state_failure" }) + "\\n");
  process.stdout.write("{}");
} else process.exit(92);
`,
  });
  assert.equal(persistenceFailure.status, 0, persistenceFailure.stderr);
  assert.match(persistenceFailure.stderr, /持久化 Review infra failure 失败/);
  assert.match(persistenceFailure.ghLog, /handoff_after_state_failure/);

  const allCommentsFail = runReviewCommand("workflow-finalize", {
    env: { REVIEW_INSTALL_OUTCOME: "failure" },
    ghScript: `
const args = process.argv.slice(2).join(" ");
if (args.includes("search/issues")) process.exit(1);
if (args.includes("issues/1148/comments")) process.exit(1);
process.exit(92);
`,
  });
  assert.equal(allCommentsFail.status, 0, allCommentsFail.stderr);
  assert.equal(allCommentsFail.reviewOutcome.kind, "infra_failure");
  assert.match(allCommentsFail.stderr, /发布 Review 降级评论失败/);
});

test("deferred finalize: 可信 job 发布 passed/gate artifact", () => {
  for (const row of [
    { outcome: { kind: "passed", gate: "APPROVED" }, step: "success", status: 0, body: "通过评论" },
    { outcome: { kind: "gate_failure", gate: "TIE" }, step: "failure", status: 1, body: "未通过评论" },
  ]) {
    const finalized = runReviewCommand("workflow-finalize", {
      env: {
        REVIEW_DEFERRED_RESULT: "1",
        REVIEW_JOB_OUTCOME: "success",
        REVIEW_INSTALL_OUTCOME: "success",
        REVIEW_CONFIGURE_OUTCOME: "success",
        REVIEW_CHECKOUT_OUTCOME: "success",
        REVIEW_STEP_OUTCOME: row.step,
      },
      outcome: row.outcome,
      comment: row.body,
      ghScript: recordingGhScript(),
    });
    assert.equal(finalized.status, row.status, finalized.stderr);
    const comments = parseGitHubJsonLines(finalized.ghLog).filter((entry) => entry.kind === "pr_comment");
    assert.equal(comments.length, 1);
    assert.equal(comments[0].body, row.body);
  }
});

test("deferred finalize: infra artifact 与缺评论文件都由可信 job handoff 且不中断", () => {
  const infra = runReviewCommand("workflow-finalize", {
    env: {
      REVIEW_DEFERRED_RESULT: "1",
      REVIEW_JOB_OUTCOME: "success",
      REVIEW_INSTALL_OUTCOME: "success",
      REVIEW_CONFIGURE_OUTCOME: "success",
      REVIEW_CHECKOUT_OUTCOME: "success",
      REVIEW_STEP_OUTCOME: "success",
    },
    outcome: { kind: "infra_failure", phase: "reviewer_execution", reason: "四路 provider 503" },
    ghScript: recordingGhScript(),
  });
  assert.equal(infra.status, 0, infra.stderr);
  const infraLog = parseGitHubJsonLines(infra.ghLog);
  assert.match(infraLog.find((entry) => entry.kind === "state_comment")?.body || "", /reviewer_execution/);
  assert.match(infraLog.find((entry) => entry.kind === "handoff_comment")?.body || "", /请忽略本次 Review Action 结果/);

  const missingComment = runReviewCommand("workflow-finalize", {
    env: {
      REVIEW_DEFERRED_RESULT: "1",
      REVIEW_JOB_OUTCOME: "success",
      REVIEW_INSTALL_OUTCOME: "success",
      REVIEW_CONFIGURE_OUTCOME: "success",
      REVIEW_CHECKOUT_OUTCOME: "success",
      REVIEW_STEP_OUTCOME: "success",
    },
    outcome: { kind: "passed", gate: "APPROVED" },
    ghScript: recordingGhScript(),
  });
  assert.equal(missingComment.status, 0, missingComment.stderr);
  assert.equal(missingComment.reviewOutcome.kind, "infra_failure");
  assert.equal(missingComment.reviewOutcome.phase, "review_comment");
  const missingLog = parseGitHubJsonLines(missingComment.ghLog);
  assert.match(missingLog.find((entry) => entry.kind === "state_comment")?.body || "", /review_comment/);
  assert.ok(missingLog.some((entry) => entry.kind === "handoff_comment"));
});

test("review panel: 4/0 与无 finding 的 3/1 通过", async () => {
  for (const mode of ["approve", "three_one_empty"]) {
    const result = await evaluatePanel(mode);
    assert.equal(result.outcome, "passed");
    assert.equal(result.gate.passed, true);
    assert.match(result.body, /\*\*通过\*\*/);
  }
});

test("review panel: 3/1 中任一真实 finding 强制失败", async () => {
  const result = await evaluatePanel("three_one");
  assert.equal(result.outcome, "gate_failure");
  assert.equal(result.gate.status, "REQUEST_CHANGES");
  assert.match(result.body, /存在 1 项真实 finding/);
  assert.match(result.body, /server\/src\/main\.rs/);
});

test("review panel: 首轮 finding 即使复投四票 APPROVE 仍保留并失败", async () => {
  const result = await evaluatePanel("first_finding_withdrawn");
  assert.equal(result.outcome, "gate_failure");
  assert.equal(result.gate.status, "REQUEST_CHANGES");
  assert.match(result.body, /存在 1 项真实 finding/);
  assert.match(result.body, /server\/src\/main\.rs/);
});

test("review panel: 2/2 保留真实 finding 并失败", async () => {
  const result = await evaluatePanel("tie");
  assert.equal(result.outcome, "gate_failure");
  assert.equal(result.gate.status, "REQUEST_CHANGES");
  assert.match(result.body, /未通过/);
  assert.match(result.body, /server\/src\/main\.rs/);
});

test("review panel: 四路 503 与零置信空 findings 都是 infra", async () => {
  for (const mode of ["infra", "zero_empty"]) {
    const result = await evaluatePanel(mode);
    assert.equal(result.outcome, "infra_failure");
    assert.equal(result.firstRound.length, 4);
    assert.ok(result.finalRound.every((row) => row.execution_failure === true));
  }
});

test("review 命令: 缺 key 只走 infra handoff 并成功退出", () => {
  const missingKey = runReviewCommand("review", {
    env: { REVIEW_CODEX_API_KEY: "", OPENAI_API_KEY: "" },
    ghScript: recordingGhScript(),
  });
  assert.equal(missingKey.status, 0, missingKey.stderr);
  assert.equal(missingKey.reviewOutcome.kind, "infra_failure");
  assert.ok(parseGitHubJsonLines(missingKey.ghLog).some((entry) => entry.kind === "handoff_comment"));
});
test("workflow: 三 job 隔离写权限、可信脚本、PR head 与 deferred artifact", () => {
  const workflow = readFileSync(new URL("../workflows/review.yml", import.meta.url), "utf8");
  const preflight = workflowJob(workflow, "preflight");
  const review = workflowJob(workflow, "review");
  const finalize = workflowJob(workflow, "finalize");

  // codex 引擎恢复为 PR 创建时的默认审核：pull_request_target 自动触发 + /review 评论 + dispatch。
  // （claude 引擎 review-claude.yml 已休眠，不自动跑；见 review-claude.yml 顶部说明。）
  assert.match(workflow, /^  pull_request_target:\n    types: \[opened\]$/m);
  assert.doesNotMatch(workflow, /^  pull_request:\n/m);
  assert.match(workflow, /^  issue_comment:\n    types: \[created\]$/m);
  assert.match(workflow, /^permissions: \{\}$/m);
  assert.match(workflow, /github\.event\.comment\.body == '\/review'/);
  assert.doesNotMatch(workflow, /startsWith\(github\.event\.comment\.body, '\/review'\)/);
  assert.match(workflow, /ref: \$\{\{ github\.event\.pull_request\.base\.sha \|\| github\.event\.repository\.default_branch \}\}/);
  assert.doesNotMatch(workflow, /REVIEW_CIRCUIT_ISSUE_NUMBER/);

  assert.match(preflight, /pull-requests: write/);
  assert.match(preflight, /issues: write/);
  assert.match(preflight, /Review 熔断预检/);
  assert.match(preflight, /continue-on-error: true/);
  assert.match(preflight, /REVIEW_TRIGGER: \$\{\{ github\.event_name \}\}/);
  assert.match(preflight, /REVIEW_COMMENT_BODY: \$\{\{ github\.event\.comment\.body \|\| '' \}\}/);
  assert.doesNotMatch(preflight, /REVIEW_CODEX_API_KEY|切到并校验 PR head/);

  assert.match(review, /contents: read/);
  assert.match(review, /pull-requests: read/);
  assert.doesNotMatch(review, /pull-requests: write|issues: write/);
  assert.match(review, /暂存可信 Review 脚本/);
  assert.ok(review.indexOf("暂存可信 Review 脚本") < review.indexOf("切到并校验 PR head"));
  assert.match(review, /gh pr checkout "\$PRNUM" --detach/);
  assert.match(review, /headRefOid/);
  assert.match(review, /git rev-parse HEAD/);
  assert.match(review, /"\$ACTUAL_HEAD" != "\$EXPECTED_HEAD"/);
  assert.doesNotMatch(review, /\|\| echo/);
  assert.match(review, /REVIEW_DEFER_COMMENT: "1"/);
  assert.match(review, /REVIEW_CODEX_BASE_URL/);
  assert.doesNotMatch(review, /REVIEW_TEST_MODE|npm install|codex --version|config\.toml/);
  assert.match(review, /actions\/upload-artifact@v4/);
  assert.match(review, /review-run-outcome\.json/);
  assert.match(review, /review\.md/);
  assert.match(review, /timeout-minutes: 40[^]*REVIEW_TOTAL_TIMEOUT_MINUTES:.*'35'/);

  assert.match(finalize, /pull-requests: write/);
  assert.match(finalize, /issues: write/);
  assert.match(finalize, /actions\/download-artifact@v4/);
  assert.match(finalize, /REVIEW_DEFERRED_RESULT: "1"/);
  assert.match(finalize, /REVIEW_JOB_OUTCOME: \$\{\{ needs\.review\.result/);
  assert.match(finalize, /REVIEW_INSTALL_OUTCOME: success/);
  assert.match(finalize, /REVIEW_CONFIGURE_OUTCOME: success/);
  assert.match(finalize, /node \.github\/scripts\/review\.mjs workflow-finalize/);
  // finalize 只在 preflight 真跑过(成功/失败/熔断)时收尾；preflight 因非触发评论被 skip 时
  // finalize 必须一并 skip——否则会去读不存在的 review artifact 崩溃(ENOENT)并误记 infra_failure。
  // (CodeRabbit 等在每个 PR 的评论都会唤醒本 workflow → preflight skip)
  assert.match(finalize, /always\(\) &&\s*needs\.preflight\.result != 'skipped' &&/);
  assert.doesNotMatch(finalize, /REVIEW_CODEX_API_KEY|gh pr checkout/);

  for (const [name, block, reserve] of [
    ["preflight", preflight, 5],
    ["review", review, 15],
    ["finalize", finalize, 5],
  ]) {
    const jobTimeout = Number(block.match(/^    timeout-minutes: (\d+)$/m)?.[1]);
    const stepTimeouts = [...block.matchAll(/^        timeout-minutes: (\d+)$/gm)].map((match) => Number(match[1]));
    assert.ok(Number.isFinite(jobTimeout), `${name} 必须有 job timeout`);
    assert.ok(stepTimeouts.length > 0, `${name} 每个外部步骤必须有 timeout`);
    assert.ok(stepTimeouts.reduce((sum, value) => sum + value, 0) <= jobTimeout - reserve, `${name} 必须保留调度余量`);
  }
});

test("review script tests workflow: Node 24 自动 gate 仅获 contents:read", () => {
  const workflow = readFileSync(new URL("../workflows/review-script-tests.yml", import.meta.url), "utf8");
  assert.match(workflow, /^  pull_request:\n    paths:$/m);
  for (const path of [
    ".github/scripts/review.mjs",
    ".github/scripts/review.test.mjs",
    ".github/workflows/review.yml",
    ".github/workflows/review-script-tests.yml",
  ]) {
    assert.match(workflow, new RegExp(`- ${path.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")}`));
  }
  assert.match(workflow, /^permissions:\n  contents: read$/m);
  assert.doesNotMatch(workflow, /pull-requests: write|issues: write/);
  assert.match(workflow, /node-version: "24"/);
  assert.match(workflow, /node --check \.github\/scripts\/review\.mjs/);
  assert.match(workflow, /node --test \.github\/scripts\/review\.test\.mjs/);
});

test("workflow permissions: 写权限仅在可信 preflight/finalize，review job 只读", () => {
  const workflow = readFileSync(new URL("../workflows/review.yml", import.meta.url), "utf8");
  const preflight = workflowJob(workflow, "preflight");
  const review = workflowJob(workflow, "review");
  const finalize = workflowJob(workflow, "finalize");
  assert.match(workflow, /^permissions: \{\}$/m);
  assert.match(preflight, /contents: read[^]*pull-requests: write[^]*issues: write/);
  assert.match(preflight, /run 29062098910.*403/);
  assert.match(preflight, /独立熔断状态 issue/);
  assert.match(review, /contents: read[^]*pull-requests: read/);
  assert.doesNotMatch(review, /: write/);
  assert.match(finalize, /contents: read[^]*pull-requests: write[^]*issues: write/);
  assert.equal([...workflow.matchAll(/^    permissions:/gm)].length, 3, "三个 job 必须显式声明最小权限");
});

test("extractJSON: 支持纯 JSON、围栏、前后废话、字符串内括号", () => {
  assert.deepEqual(extractJSON('{"a":1}'), { a: 1 });
  assert.deepEqual(extractJSON('```json\n{"a":1}\n```'), { a: 1 });
  assert.deepEqual(extractJSON('前缀 {"a":[1,2]} 后缀'), { a: [1, 2] });
  assert.deepEqual(extractJSON('{"s":"含 } 的字符串"}'), { s: "含 } 的字符串" });
  assert.equal(extractJSON("no json"), null);
});

test("normalizeVote: 只有 APPROVE 算通过票，其余从严视为 REQUEST_CHANGES", () => {
  assert.equal(normalizeVote("APPROVE"), "APPROVE");
  assert.equal(normalizeVote("approve"), "APPROVE");
  assert.equal(normalizeVote("REQUEST_CHANGES"), "REQUEST_CHANGES");
  assert.equal(normalizeVote("maybe"), "REQUEST_CHANGES");
  assert.equal(normalizeVote(undefined), "REQUEST_CHANGES");
});

test("normalizePlanStatus: 只接受四种状态", () => {
  assert.equal(normalizePlanStatus("aligned"), "aligned");
  assert.equal(normalizePlanStatus("misaligned"), "misaligned");
  assert.equal(normalizePlanStatus("not_plan"), "not_plan");
  assert.equal(normalizePlanStatus("unclear"), "unclear");
  assert.equal(normalizePlanStatus("other"), "unclear");
});

test("decideGate: 4 人面板只有 3/1 或 4/0 approve 才通过，2/2 不通过", () => {
  const approve = { vote: "APPROVE" };
  const request = { vote: "REQUEST_CHANGES" };
  assert.deepEqual(decideGate([approve, approve, approve, approve]).status, "APPROVED");
  assert.deepEqual(decideGate([approve, approve, approve, request]).status, "APPROVED");
  assert.deepEqual(decideGate([approve, approve, request, request]).status, "TIE");
  assert.equal(decideGate([approve, approve, request, request]).passed, false);
  assert.deepEqual(decideGate([approve, request, request, request]).status, "REQUEST_CHANGES");
});

test("applyPlanIntentGate: 有关联 plan 时未 aligned 的票强制 REQUEST_CHANGES", () => {
  const rows = [
    { vote: "APPROVE", summary: "ok", plan_intent: { status: "aligned", reason: "符合" } },
    { vote: "APPROVE", summary: "ok", plan_intent: { status: "unclear", reason: "没找到验收项" } },
    { vote: "APPROVE", summary: "", plan_intent: { status: "misaligned", reason: "缺测试" } },
  ];
  const gated = applyPlanIntentGate(rows, true);
  assert.equal(gated[0].vote, "APPROVE");
  assert.equal(gated[1].vote, "REQUEST_CHANGES");
  assert.match(gated[1].summary, /Plan 原意未确认/);
  assert.equal(gated[2].vote, "REQUEST_CHANGES");
  assert.deepEqual(applyPlanIntentGate(rows, false), rows, "非 plan PR 不强制 plan gate");
});

test("findPlanName: 从标题/分支/body 或变更文件里识别 plan", () => {
  assert.equal(findPlanName({ title: "实现 plan-foo-v1", files: [] }), "plan-foo-v1");
  assert.equal(findPlanName({ headRefName: "fix/plan-bar-v2", files: [] }), "plan-bar-v2");
  assert.equal(findPlanName({ body: "refs plan-baz-v3", files: [] }), "plan-baz-v3");
  assert.equal(
    findPlanName({ title: "修复制作退款", files: [{ path: "docs/plan-bughunt-craft-refund-full-inventory-loss-v1.md" }] }),
    "plan-bughunt-craft-refund-full-inventory-loss-v1",
  );
  assert.equal(findPlanName({ title: "no plan", files: [{ path: "server/src/foo.rs" }] }), null);
});

test("workspace file: 只读取 workspace 内普通文件，拒绝 symlink 与目录外逃", () => {
  const directory = mkdtempSync(join(tmpdir(), "bong-review-workspace-"));
  try {
    mkdirSync(join(directory, "docs"), { recursive: true });
    writeFileSync(join(directory, "docs", "plan-safe-v1.md"), "safe plan");
    symlinkSync("/proc/self/environ", join(directory, "docs", "plan-leak-v1.md"));
    writeFileSync(join(directory, "outside.md"), "outside");

    assert.equal(readWorkspaceRegularFile("docs/plan-safe-v1.md", directory), "safe plan");
    assert.equal(readWorkspaceRegularFile("docs/plan-leak-v1.md", directory), null);
    assert.equal(readWorkspaceRegularFile("../outside.md", directory), null);
    assert.equal(readWorkspaceRegularFile("docs/missing.md", directory), null);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("normalizeResult: 合法 JSON 归一字段，非法输出按未通过处理", () => {
  const ok = normalizeResult(
    JSON.stringify({
      reviewer: "A",
      vote: "APPROVE",
      confidence: 88.7,
      plan_intent: { status: "aligned", reason: "符合", missing: ["无"] },
      summary: "可合并",
      findings: [{ severity: "major", file: "a.rs", line: "10", title: "问题" }],
    }),
    reviewer,
  );
  assert.equal(ok.vote, "APPROVE");
  assert.equal(ok.confidence, 89);
  assert.equal(ok.plan_intent.status, "aligned");
  assert.equal(ok.findings[0].severity, "major");

  const bad = normalizeResult("不是 json", reviewer);
  assert.equal(bad.vote, "REQUEST_CHANGES");
  assert.equal(bad.confidence, 0);
  assert.equal(bad.plan_intent.status, "unclear");
  assert.equal(bad.findings.length, 1);
});

test("mergeFindings: 同 file/line/title 合并 reviewer，取更高严重度和更长证据", () => {
  const out = mergeFindings([
    {
      reviewer: "A",
      findings: [{ severity: "minor", file: "a.rs", line: "10", title: "Bug", evidence: "短" }],
    },
    {
      reviewer: "B",
      findings: [{ severity: "blocker", file: "a.rs", line: "10", title: "bug", evidence: "更长的证据" }],
    },
    {
      reviewer: "C",
      findings: [{ severity: "major", file: "b.rs", line: "3", title: "Other" }],
    },
  ]);
  assert.equal(out.length, 2);
  assert.equal(out[0].severity, "blocker");
  assert.deepEqual(out[0].reviewers, ["A", "B"]);
  assert.equal(out[0].evidence, "更长的证据");
});

test("codexFailureText: 失败摘要保留 exit 与 stderr 头尾", () => {
  const long = `HEAD-${"x".repeat(2000)}-TAIL`;
  const text = codexFailureText({ code: 1, signal: null, stderr: long, stdout: "" });
  assert.match(text, /exit=1/);
  assert.match(text, /HEAD-/);
  assert.match(text, /-TAIL/);
  assert.match(text, /truncated/);
  assert.equal(
    redactRuntimeSecrets("token=provider-secret", { REVIEW_CODEX_API_KEY: "provider-secret" }),
    "token=[REDACTED]",
  );
});

test("provider retry delay: 429 至少退避 120 秒，其他暂时故障保留递增配置", () => {
  assert.equal(codexRetryDelayMs({ code: 429 }, 1, 1_000), 120_000);
  assert.equal(codexRetryDelayMs({ code: 429 }, 2, 70_000), 140_000);
  assert.equal(codexRetryDelayMs({ code: 503 }, 1, 15_000), 15_000);
  assert.equal(codexRetryDelayMs({ code: 124 }, 3, 15_000), 45_000);
});

test("isRetryableCodexFailure: 仅重试限流、上游暂时失败和超时", () => {
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "429 Too Many Requests" }), true);
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "channel is temporarily unavailable; upstream_400" }), true);
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "503 Service Unavailable" }), true);
  assert.equal(isRetryableCodexFailure({ code: 524, stderr: "" }), true);
  assert.equal(isRetryableCodexFailure({ code: 124, stderr: "" }), true);
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "invalid API key" }), false);
});

test("excerptLog: 短文本不改，长文本保留头尾", () => {
  assert.equal(excerptLog("abc", 10), "abc");
  const out = excerptLog(`A${"b".repeat(100)}Z`, 40);
  assert.match(out, /^A/);
  assert.match(out, /Z$/);
});
