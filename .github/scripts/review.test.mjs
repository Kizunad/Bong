// Review v3 纯逻辑测试 —— `node --test .github/scripts/review.test.mjs`

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  applyPlanIntentGate,
  decideGate,
  extractJSON,
  mergeFindings,
  normalizePlanStatus,
  normalizeResult,
  normalizeVote,
} from "./review.mjs";

const reviewer = { id: "A", name: "Plan 原意核查" };

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
