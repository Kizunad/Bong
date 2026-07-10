// Review v3 纯逻辑测试 —— `node --test .github/scripts/review.test.mjs`

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  applyPlanIntentGate,
  classifyReviewRun,
  codexFailureText,
  decideGate,
  evaluateCircuit,
  excerptLog,
  extractJSON,
  findPlanName,
  isManualReviewTrigger,
  isRetryableCodexFailure,
  mergeFindings,
  normalizePlanStatus,
  normalizeResult,
  normalizeVote,
  parseHiddenMarkers,
  redactCodexPromptEcho,
  renderCircuitSkipComment,
  renderHiddenMarker,
  renderInfrastructureHandoffComment,
} from "./review.mjs";

const reviewer = { id: "A", name: "Plan 原意核查" };

const infraResult = {
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
  assert.match(body, /本次 Action review 结果请忽略/);
  assert.match(body, /改走 agent 自己的博弈式 review 流程，并会向用户反馈/);
  assert.match(body, /\/review/);
  assert.deepEqual(parseHiddenMarkers(body, "bong-review-handoff"), [event]);
});

test("熔断跳过评论: 明示截止时间、成功退出和 /review 手动旁路", () => {
  const body = renderCircuitSkipComment({ open: true, openUntil: "2026-07-10T01:00:00.000Z" });
  assert.match(body, /快速跳过并成功退出，不影响其他 CI/);
  assert.match(body, /2026-07-10T01:00:00.000Z/);
  assert.match(body, /\/review/);
  assert.equal(parseHiddenMarkers(body, "bong-review-handoff")[0].kind, "circuit_skip");
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

test("manual bypass: 评论 /review 与 workflow_dispatch 始终旁路", () => {
  assert.equal(isManualReviewTrigger("pull_request"), false);
  assert.equal(isManualReviewTrigger("issue_comment"), true);
  assert.equal(isManualReviewTrigger("workflow_dispatch"), true);
});

test("infra-only classification: reviewer 执行失败才算 infra，真实 REQUEST_CHANGES 不计数", () => {
  assert.equal(classifyReviewRun([infraResult], [realRequestChanges]), "infra_failure");
  assert.equal(classifyReviewRun([realRequestChanges], [realRequestChanges]), "gate_failure");
  const approve = { vote: "APPROVE", confidence: 90, findings: [] };
  assert.equal(classifyReviewRun([approve], [approve, approve, approve, realRequestChanges]), "passed");
});

test("infra-only classification: 模型给出真实审查或不可解析内容都不伪造 infra", () => {
  const malformed = normalizeResult("不是 JSON", reviewer);
  assert.equal(classifyReviewRun([malformed], [malformed]), "gate_failure");
});

test("workflow: 预检先于 CLI，自动失败统一降级，manual trigger 仍进入预检旁路", () => {
  const workflow = readFileSync(new URL("../workflows/review.yml", import.meta.url), "utf8");
  assert.ok(workflow.indexOf("Review 熔断预检") < workflow.indexOf("安装 Codex CLI"));
  assert.match(workflow, /REVIEW_TRIGGER: \$\{\{ github\.event_name \}\}/);
  assert.match(workflow, /continue-on-error: true/);
  assert.match(workflow, /workflow-finalize/);
  assert.match(workflow, /REVIEW_INFRA_FAILURE_THRESHOLD.*'3'/);
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
