// Review v4(claude) 纯逻辑测试 —— `node --test .github/scripts/review-claude.test.mjs`

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  claudeFailureText,
  clampConcurrency,
  deriveGate,
  excerptLog,
  extractJSON,
  findPlanName,
  mergeFindings,
  normalizeDecision,
  normalizePlanStatus,
  normalizeReviewerResult,
  normalizeVote,
  parseClaudeEnvelope,
  planAlignedFromDimensions,
} from "./review-claude.mjs";

const dimension = { id: "C", name: "正确性与守恒核查" };
const prosecutor = { id: "prosecutor", name: "控方" };

test("clampConcurrency: 硬钳到 [1, hardMax]，绝不放大，非法回落到 1", () => {
  assert.equal(clampConcurrency(2, 3), 2);
  assert.equal(clampConcurrency(99, 3), 3, "超过硬顶被钳到硬顶");
  assert.equal(clampConcurrency(0, 3), 1, "0 回落到 1");
  assert.equal(clampConcurrency(-5, 3), 1);
  assert.equal(clampConcurrency(NaN, 3), 1);
  assert.equal(clampConcurrency(2, 0), 1, "非法硬顶回落到 1");
  assert.equal(clampConcurrency(2.9, 3), 2, "小数向下取整");
});

test("extractJSON: 纯 JSON / 围栏 / 前后废话 / 字符串内括号", () => {
  assert.deepEqual(extractJSON('{"a":1}'), { a: 1 });
  assert.deepEqual(extractJSON('```json\n{"a":1}\n```'), { a: 1 });
  assert.deepEqual(extractJSON('前缀 {"a":[1,2]} 后缀'), { a: [1, 2] });
  assert.deepEqual(extractJSON('{"s":"含 } 的字符串"}'), { s: "含 } 的字符串" });
  assert.equal(extractJSON("no json"), null);
});

test("parseClaudeEnvelope: 从 --output-format json 信封里取 result；识别 is_error", () => {
  const good = JSON.stringify({ type: "result", subtype: "success", is_error: false, result: '{"vote":"APPROVE"}' });
  const ok = parseClaudeEnvelope(`⚠ some warning\n${good}`);
  assert.equal(ok.ok, true);
  assert.equal(ok.result, '{"vote":"APPROVE"}');

  const errEnv = JSON.stringify({ type: "result", is_error: true, result: "boom" });
  const bad = parseClaudeEnvelope(errEnv);
  assert.equal(bad.ok, false);
  assert.equal(bad.result, "boom");

  assert.equal(parseClaudeEnvelope("纯噪声没有 json").ok, false);
  assert.equal(parseClaudeEnvelope("").result, "");
});

test("normalizeVote / normalizePlanStatus: 从严归一", () => {
  assert.equal(normalizeVote("approve"), "APPROVE");
  assert.equal(normalizeVote("maybe"), "REQUEST_CHANGES");
  assert.equal(normalizePlanStatus("aligned"), "aligned");
  assert.equal(normalizePlanStatus("other"), "unclear");
});

test("normalizeReviewerResult: 合法 JSON 带 dimension/role，非法输出按未通过兜底", () => {
  const ok = normalizeReviewerResult(
    JSON.stringify({
      vote: "APPROVE",
      confidence: 91.4,
      plan_intent: { status: "not_plan", reason: "无", missing: [] },
      summary: "ok",
      findings: [{ severity: "minor", file: "a.rs", line: "3", title: "t" }],
    }),
    dimension,
    prosecutor,
  );
  assert.equal(ok.dimension, "C");
  assert.equal(ok.role, "prosecutor");
  assert.equal(ok.vote, "APPROVE");
  assert.equal(ok.confidence, 91);
  assert.equal(ok.parseFailed, false);

  const bad = normalizeReviewerResult("不是 json", dimension, prosecutor);
  assert.equal(bad.vote, "REQUEST_CHANGES");
  assert.equal(bad.confidence, 0);
  assert.equal(bad.parseFailed, true);
  assert.equal(bad.findings.length, 1);
  assert.equal(bad.findings[0].severity, "major");
});

test("normalizeDecision: 决策裁决归一；无法解析时从严", () => {
  const ok = normalizeDecision(
    JSON.stringify({
      vote: "APPROVE",
      confidence: 80,
      plan_intent: { status: "aligned", reason: "符合" },
      rationale: "采纳辩方",
      blocking: [],
    }),
  );
  assert.equal(ok.ok, true);
  assert.equal(ok.vote, "APPROVE");
  assert.equal(ok.plan_status, "aligned");

  const bad = normalizeDecision("坏输出");
  assert.equal(bad.ok, false);
  assert.equal(bad.vote, "REQUEST_CHANGES");
});

test("planAlignedFromDimensions: 维度 A 双方都 aligned 才算对齐", () => {
  const aligned = [
    { id: "A", opinions: [{ plan_intent: { status: "aligned" } }, { plan_intent: { status: "aligned" } }] },
  ];
  assert.equal(planAlignedFromDimensions(aligned), true);

  const oneUnclear = [
    { id: "A", opinions: [{ plan_intent: { status: "aligned" } }, { plan_intent: { status: "unclear" } }] },
  ];
  assert.equal(planAlignedFromDimensions(oneUnclear), false);

  assert.equal(planAlignedFromDimensions([{ id: "A", opinions: [] }]), false, "无意见不算对齐");
  assert.equal(planAlignedFromDimensions([{ id: "B", opinions: [] }]), false, "缺维度 A 不算对齐");
});

test("deriveGate: 决策 APPROVE 且无 plan → 通过", () => {
  const decision = { ok: true, vote: "APPROVE", rationale: "可合并", blocking: [] };
  const dims = [{ id: "A", opinions: [{ vote: "APPROVE", findings: [] }] }];
  const gate = deriveGate(decision, dims, false);
  assert.equal(gate.passed, true);
  assert.equal(gate.status, "APPROVED");
});

test("deriveGate: 有 plan 但维度 A 未双方 aligned → 强制不通过，即便决策 APPROVE", () => {
  const decision = { ok: true, vote: "APPROVE", rationale: "看着行", blocking: [] };
  const dims = [
    { id: "A", opinions: [{ vote: "APPROVE", plan_intent: { status: "aligned" }, findings: [] }, { vote: "APPROVE", plan_intent: { status: "unclear" }, findings: [] }] },
  ];
  const gate = deriveGate(decision, dims, true);
  assert.equal(gate.passed, false);
  assert.match(gate.reason, /Plan 原意未被双方确认/);
});

test("deriveGate: 决策失联 → 回落确定性(无 blocker/major 且 plan 对齐才过)", () => {
  const failed = { ok: false, vote: "REQUEST_CHANGES", rationale: "", blocking: [] };
  const clean = [{ id: "A", opinions: [{ vote: "APPROVE", plan_intent: { status: "aligned" }, findings: [] }, { vote: "APPROVE", plan_intent: { status: "aligned" }, findings: [] }] }];
  assert.equal(deriveGate(failed, clean, true).passed, true, "无 blocker + plan 对齐 → 回落也通过");

  const withBlocker = [
    { id: "A", opinions: [{ vote: "APPROVE", plan_intent: { status: "aligned" }, findings: [{ severity: "blocker", file: "a.rs", line: "1", title: "崩" }] }] },
  ];
  assert.equal(deriveGate(failed, withBlocker, false).passed, false, "有 blocker → 回落不通过");
});

test("mergeFindings: 同 file/line/title 合并来源标签，取更高严重度与更长证据", () => {
  const out = mergeFindings([
    { dimension: "C", role: "prosecutor", findings: [{ severity: "minor", file: "a.rs", line: "10", title: "Bug", evidence: "短" }] },
    { dimension: "C", role: "defender", findings: [{ severity: "blocker", file: "a.rs", line: "10", title: "bug", evidence: "更长的证据" }] },
    { dimension: "D", role: "prosecutor", findings: [{ severity: "major", file: "b.rs", line: "3", title: "Other" }] },
  ]);
  assert.equal(out.length, 2);
  assert.equal(out[0].severity, "blocker");
  assert.deepEqual(out[0].reviewers, ["C/prosecutor", "C/defender"]);
  assert.equal(out[0].evidence, "更长的证据");
});

test("findPlanName: 从标题/分支/body/变更文件识别 plan", () => {
  assert.equal(findPlanName({ title: "实现 plan-foo-v1", files: [] }), "plan-foo-v1");
  assert.equal(findPlanName({ headRefName: "fix/plan-bar-v2", files: [] }), "plan-bar-v2");
  assert.equal(findPlanName({ files: [{ path: "docs/plan-baz-qux-v3.md" }] }), "plan-baz-qux-v3");
  assert.equal(findPlanName({ title: "no plan", files: [{ path: "server/src/foo.rs" }] }), null);
});

test("claudeFailureText / excerptLog: 失败摘要保留 exit 与头尾", () => {
  const long = `HEAD-${"x".repeat(3000)}-TAIL`;
  const text = claudeFailureText({ code: 124, signal: "SIGTERM", stderr: long, stdout: "" });
  assert.match(text, /exit=124/);
  assert.match(text, /signal=SIGTERM/);
  assert.match(text, /HEAD-/);
  assert.match(text, /-TAIL/);
  assert.match(text, /truncated/);

  assert.equal(excerptLog("abc", 10), "abc");
});
