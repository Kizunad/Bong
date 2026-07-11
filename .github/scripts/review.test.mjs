// Review v3 纯逻辑测试 —— `node --test .github/scripts/review.test.mjs`

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  applyPlanIntentGate,
  boundedAttemptTimeout,
  buildCircuitStateSearchQuery,
  circuitGhTimeout,
  circuitOperationDeadlines,
  classifyReviewRun,
  classifyWorkflowFinalization,
  codexFailureText,
  decideGate,
  evaluateCircuit,
  ensureCircuitStateIssues,
  excerptLog,
  extractJSON,
  findCircuitStateIssues,
  findPlanName,
  isCircuitBypassTrigger,
  isRetryableCodexFailure,
  mergeFindings,
  normalizePlanStatus,
  normalizeResult,
  normalizeVote,
  normalizeCircuitEvent,
  parseGitHubJsonLines,
  parseHiddenMarkers,
  parseTrustedCircuitEvents,
  redactCodexPromptEcho,
  renderCircuitSkipComment,
  renderHiddenMarker,
  renderInfrastructureHandoffComment,
  reviewFindingResults,
  resolveCircuitStateIssueNumbers,
  selectCircuitStateIssues,
  spawnCodex,
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
    { body: marker("100", "2026-07-10T00:00:00Z"), user: { login: "attacker", type: "User" } },
    {
      body: `${marker("101", "2026-07-10T00:01:00Z")}\n${marker("102", "2026-07-10T00:02:00Z")}`,
      user: { login: "github-actions[bot]", type: "Bot" },
    },
    { body: marker("101", "2026-07-10T00:03:00Z"), user: { login: "github-actions[bot]", type: "Bot" } },
    { body: marker("103", "2026-07-10T00:04:00Z"), user: { login: "github-actions[bot]", type: "User" } },
    { body: marker("104", "2026-07-10T00:05:00Z"), user: { login: "github-actions[bot]", type: "Bot" } },
  ];
  assert.deepEqual(parseTrustedCircuitEvents(comments).map((event) => event.run_id), ["101", "104"]);
});


test("circuit event validation: 当前轮与跨 run 对空/非法 run_id 使用同一规则", () => {
  const base = { v: 1, kind: "infra_failure", at: "2026-07-10T00:00:00Z" };
  assert.equal(normalizeCircuitEvent({ ...base, run_id: "" }), null);
  assert.equal(normalizeCircuitEvent({ ...base, run_id: "abc" }), null);
  assert.equal(normalizeCircuitEvent({ ...base, run_id: "123" }).run_id, "123");
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

test("findCircuitStateIssues: Search API 标题限流并保留 duplicate resolution，绝不全仓扫描", () => {
  const body = "<!-- bong-review-circuit-state:v1 -->";
  const bot = { login: "github-actions[bot]", type: "Bot" };
  let calls = 0;
  let calledArgs;
  const found = findCircuitStateIssues("Kizunad/Bong", (args) => {
    calls += 1;
    calledArgs = args;
    return [
      { number: 20, title: "[automation] Review infrastructure circuit state", body, user: bot },
      { number: 3, title: "[automation] Review infrastructure circuit state", body, user: bot },
      { number: 4, title: "[automation] Review infrastructure circuit state", body, user: bot, pull_request: {} },
    ]
      .map(JSON.stringify)
      .join("\n");
  });

  assert.deepEqual(found, ["3", "20"]);
  assert.equal(calls, 1, "正常命中不得产生额外 Search 请求");
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

test("findCircuitStateIssues: 最终一致性延迟按约 4 RPM 固定退避，命中后立即停止", () => {
  const body = "<!-- bong-review-circuit-state:v1 -->";
  const bot = { login: "github-actions[bot]", type: "Bot" };
  const outputs = ["", JSON.stringify({ number: 4, title: "同名 PR", body, user: bot, pull_request: {} }), JSON.stringify({
    number: 20,
    title: "[automation] Review infrastructure circuit state",
    body,
    user: bot,
  })];
  const waits = [];
  let calls = 0;

  const found = findCircuitStateIssues(
    "Kizunad/Bong",
    () => outputs[calls++] ?? "",
    (ms) => waits.push(ms),
  );

  assert.deepEqual(found, ["20"]);
  assert.equal(calls, 3);
  assert.deepEqual(waits, [15_000, 15_000]);
});

test("findCircuitStateIssues: 持续空结果最多四次请求后返回空集，维持上层 fail-open", () => {
  const waits = [];
  let calls = 0;
  const found = findCircuitStateIssues(
    "Kizunad/Bong",
    () => {
      calls += 1;
      return "";
    },
    (ms) => waits.push(ms),
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
  const runGh = (args, timeoutMs) => {
    assert.ok(timeoutMs > 0 && timeoutMs <= 30_000);
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
  assert.equal(searchCalls, 5, "创建前四次空结果，创建后首次延迟命中即停止");
  assert.ok(nowMs < 120_000);
});

test("review 总预算: 单次 timeout 被剩余预算截断，并预留清理时间", () => {
  assert.equal(boundedAttemptTimeout(900_000, 1_000_000), 820_000);
  assert.equal(boundedAttemptTimeout(900_000, 180_000), 0);
  assert.equal(boundedAttemptTimeout(900_000, 60_000, 15_000), 45_000);
});

test("Codex timeout: 组长先退出后仍强制清理忽略 TERM 的后代", { skip: process.platform === "win32" }, async () => {
  const script = `(trap '' TERM; exec sleep 60) </dev/null >/dev/null 2>&1 & echo $!; trap 'exit 0' TERM; wait`;
  const result = await spawnCodex(["-c", script], "", 20, "bash", { killGraceMs: 50, forceResolveMs: 150 });
  const descendant = Number(result.stdout.trim().split(/\s+/)[0]);
  await new Promise((resolve) => setTimeout(resolve, 120));

  let running = false;
  try {
    const stat = readFileSync(`/proc/${descendant}/stat`, "utf8");
    running = !/\) Z /.test(stat);
  } catch {
    running = false;
  }
  if (running) {
    try {
      process.kill(descendant, "SIGKILL");
    } catch {}
  }

  assert.equal(result.code, 124);
  assert.equal(running, false);
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
  assert.equal(classifyReviewRun([approve], [approve, approve, approve, realRequestChanges]), "passed");
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

test("mixed review findings: 复投执行失败时保留同路首轮代码 finding", () => {
  const firstRound = [realRequestChanges, infraResult, realRequestChanges, infraResult];
  const finalRound = [infraResult, infraResult, realRequestChanges, infraResult];
  const retained = reviewFindingResults(firstRound, finalRound);
  assert.deepEqual(retained.slice(0, 4), finalRound);
  assert.equal(retained.length, 5);
  assert.equal(retained[4], realRequestChanges);
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
    assert.equal(classifyWorkflowFinalization({ install: outcome }).phase, "cli_install");
    assert.equal(classifyWorkflowFinalization({ install: "success", configure: outcome }).phase, "provider_config");
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
test("workflow: 预检先于 CLI，自动失败统一降级，manual trigger 仍进入预检旁路", () => {
  const workflow = readFileSync(new URL("../workflows/review.yml", import.meta.url), "utf8");
  assert.ok(workflow.indexOf("Review 熔断预检") < workflow.indexOf("安装 Codex CLI"));
  assert.match(workflow, /REVIEW_TRIGGER: \$\{\{ github\.event_name \}\}/);
  assert.match(workflow, /REVIEW_COMMENT_BODY: \$\{\{ github\.event\.comment\.body \|\| '' \}\}/);
  assert.match(workflow, /github\.event\.comment\.body == '\/review'/);
  assert.doesNotMatch(workflow, /startsWith\(github\.event\.comment\.body, '\/review'\)/);
  assert.match(workflow, /continue-on-error: true/);
  assert.match(workflow, /workflow-finalize/);
  assert.match(workflow, /REVIEW_INFRA_FAILURE_THRESHOLD.*'3'/);
  assert.match(workflow, /REVIEW_TOTAL_TIMEOUT_MINUTES.*'35'/);
  assert.match(workflow, /REVIEW_GH_TIMEOUT_MS.*'30000'/);
  assert.doesNotMatch(workflow, /REVIEW_CIRCUIT_ISSUE_NUMBER/);
  const jobTimeout = Number(workflow.match(/^    timeout-minutes: (\d+)$/m)[1]);
  const stepTimeouts = [...workflow.matchAll(/^        timeout-minutes: (\d+)$/gm)].map((match) => Number(match[1]));
  assert.equal(stepTimeouts.length, 8, "每个 workflow step 都必须有独立 timeout");
  assert.ok(stepTimeouts.reduce((sum, value) => sum + value, 0) <= jobTimeout - 15, "必须给调度与 finalize 留至少 15 分钟");
  assert.match(workflow, /timeout-minutes: 40[^]*REVIEW_TOTAL_TIMEOUT_MINUTES:.*'35'/);
});

test("workflow permissions: PR 评论与独立状态 issue 仅获各自必要写权限并记录依据", () => {
  const workflow = readFileSync(new URL("../workflows/review.yml", import.meta.url), "utf8");
  const block = workflow.match(/^permissions:\n((?:  (?:#.*|[a-z-]+:.*)\n)+)/m)?.[1] || "";
  const entries = Object.fromEntries(
    [...block.matchAll(/^  ([a-z-]+):\s*(read|write|none)\s*(?:#.*)?$/gm)].map((match) => [match[1], match[2]]),
  );

  assert.deepEqual(entries, { contents: "read", "pull-requests": "write", issues: "write" });
  assert.doesNotMatch(block, /pull-requests:\s*read/);
  assert.match(block, /run 29062098910.*403/);
  assert.match(block, /独立熔断状态 issue/);
  assert.equal([...workflow.matchAll(/^permissions:/gm)].length, 1, "权限必须集中在顶层，避免 job 级覆盖漂移");
  assert.equal([...workflow.matchAll(/^\s{2,}permissions:/gm)].length, 0, "job/step 不得覆盖最小权限矩阵");
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

test("redactCodexPromptEcho: 删除 Codex stderr 里的用户 prompt echo，保留真实错误", () => {
  const stderr = [
    "Reading additional input from stdin...",
    "OpenAI Codex v0.143.0",
    "--------",
    "user",
    "很长的 diff",
    "+ dangerous prompt echo",
    "ERROR: stream disconnected before completion: status code 401",
  ].join("\n");

  const redacted = redactCodexPromptEcho(stderr);
  assert.match(redacted, /\[prompt echo omitted\]/);
  assert.doesNotMatch(redacted, /dangerous prompt echo/);
  assert.match(redacted, /status code 401/);
});

test("codexFailureText: 失败摘要保留 exit 与 stderr 头尾", () => {
  const long = `HEAD-${"x".repeat(2000)}-TAIL`;
  const text = codexFailureText({ code: 1, signal: null, stderr: long, stdout: "" });
  assert.match(text, /exit=1/);
  assert.match(text, /HEAD-/);
  assert.match(text, /-TAIL/);
  assert.match(text, /truncated/);
});

test("isRetryableCodexFailure: 仅重试限流、上游暂时失败和超时", () => {
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "429 Too Many Requests" }), true);
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "channel is temporarily unavailable; upstream_400" }), true);
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "503 Service Unavailable" }), true);
  assert.equal(isRetryableCodexFailure({ code: 124, stderr: "" }), true);
  assert.equal(isRetryableCodexFailure({ code: 1, stderr: "invalid API key" }), false);
});

test("excerptLog: 短文本不改，长文本保留头尾", () => {
  assert.equal(excerptLog("abc", 10), "abc");
  const out = excerptLog(`A${"b".repeat(100)}Z`, 40);
  assert.match(out, /^A/);
  assert.match(out, /Z$/);
});
