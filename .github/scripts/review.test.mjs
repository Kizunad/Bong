// 统一 Review reviewer 纯逻辑测试 —— `node --test .github/scripts/review.test.mjs`
// 锁住决策行为:模型→harness 路由、自信度聚合(收敛门控)、倒计时分档、findings 去重、
// 怀疑投票分流(accept/reject/pending + 末轮从严)、JSON/成员解析容错、动态分档、补充文件路径安全。
// 这些是「确定性外壳」的核心——任何回归(路由错判、分流阈值错算、去重漏合并、末轮放水)都要立刻撞红。

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  harnessOf,
  normalizeConfidence,
  aggregateConfidence,
  computeCountdown,
  dedupeFindings,
  dedupeUncertain,
  tallyDecision,
  extractJSON,
  parseMember,
  isSafeRepoPath,
  expandModelList,
  pickTier,
  buildTierPanel,
  TIERS,
} from "./review.mjs";

// ── harnessOf:模型 → harness 路由(核心)────────────────────────────────────────
test("harnessOf: GPT 系走 codex,其余走 claude", () => {
  assert.equal(harnessOf("gpt-5.5"), "codex", "gpt-5.5 → codex");
  assert.equal(harnessOf("gpt-4o"), "codex", "gpt-4o → codex");
  assert.equal(harnessOf("o1"), "codex", "o1 → codex");
  assert.equal(harnessOf("o3-mini"), "codex", "o3 → codex");
  assert.equal(harnessOf("openai/gpt-5.5"), "codex", "openai 前缀 → codex");
  assert.equal(harnessOf("chatgpt-4o-latest"), "codex", "chatgpt → codex");
  assert.equal(harnessOf("codex-mini"), "codex", "codex → codex");
  assert.equal(harnessOf("deepseek-v4-flash"), "claude", "deepseek → claude");
  assert.equal(harnessOf("sensenova-6.7-flash-lite"), "claude", "sensenova → claude");
  assert.equal(harnessOf("claude-sonnet-4-6"), "claude", "claude → claude");
  assert.equal(harnessOf("minimax-m3"), "claude", "其它小模型 → claude");
  assert.equal(harnessOf(""), "claude", "空 → claude(默认)");
  assert.equal(harnessOf(undefined), "claude", "undefined → claude(默认)");
  assert.equal(harnessOf("  gpt-5.5  "), "codex", "前后空白不影响判定");
});

// ── normalizeConfidence ──────────────────────────────────────────────────────
test("normalizeConfidence: 整数/字符串/边界/越界/非法", () => {
  assert.equal(normalizeConfidence(80), 80);
  assert.equal(normalizeConfidence("75"), 75);
  assert.equal(normalizeConfidence(0), 0, "下边界 0 合法");
  assert.equal(normalizeConfidence(100), 100, "上边界 100 合法");
  assert.equal(normalizeConfidence(150), 100, "越上界钳到 100");
  assert.equal(normalizeConfidence(-5), 0, "越下界钳到 0");
  assert.equal(normalizeConfidence(82.6), 83, "四舍五入");
  assert.equal(normalizeConfidence(NaN), null, "NaN → null");
  assert.equal(normalizeConfidence(undefined), null);
  assert.equal(normalizeConfidence("abc"), null);
  assert.equal(normalizeConfidence(undefined, 50), 50, "fallback 可指定");
});

// ── aggregateConfidence:中位数收敛门控判据 ───────────────────────────────────
test("aggregateConfidence: 奇偶中位数 / 过滤非法 / 全空", () => {
  assert.deepEqual(aggregateConfidence([70, 80, 90]), { median: 80, min: 70, max: 90, n: 3 }, "奇数取中");
  assert.deepEqual(aggregateConfidence([60, 80]), { median: 70, min: 60, max: 80, n: 2 }, "偶数取两中均值");
  assert.deepEqual(aggregateConfidence([80, null, "x", 90]), { median: 85, min: 80, max: 90, n: 2 }, "非法项被过滤");
  assert.deepEqual(aggregateConfidence([]), { median: null, min: null, max: null, n: 0 }, "全空 → null");
  assert.equal(aggregateConfidence([50]).median, 50, "单值");
});

// ── computeCountdown:倒计时语气逐级加码 ──────────────────────────────────────
test("computeCountdown: 单轮 / 首轮 / 中段 / 倒数第二 / 最终轮", () => {
  assert.equal(computeCountdown(1, 1).left, 0, "单轮 left=0");
  assert.match(computeCountdown(1, 1).text, /唯一一轮/);
  assert.equal(computeCountdown(1, 3).left, 2, "首轮 left=2");
  assert.match(computeCountdown(1, 3).text, /从容核查/);
  assert.match(computeCountdown(2, 4).text, /还剩 2 轮/, "中段报剩余");
  assert.match(computeCountdown(3, 4).text, /只剩 1 轮/, "倒数第二轮告警");
  assert.equal(computeCountdown(4, 4).left, 0, "最终轮 left=0");
  assert.match(computeCountdown(4, 4).text, /最终轮/);
});

// ── dedupeFindings:去重 + consensus + 严重度合并 + 排序 + id 注入前的稳定性 ────
test("dedupeFindings: 同坐标合并 consensus / 取最高严重度 / 取最长证据 / 严重度排序", () => {
  const out = dedupeFindings([
    { severity: "minor", file: "a.rs", line: "10", title: "Bug X", evidence: "short", why: "w1", by: "r1#0" },
    { severity: "blocker", file: "a.rs", line: "10", title: "bug x", evidence: "longer evidence", why: "", by: "r1#1" },
    { severity: "major", file: "b.rs", line: "5", title: "Bug Y", by: "r1#2" },
  ]);
  assert.equal(out.length, 2, "a.rs:10 两条合并成一条");
  const x = out.find((f) => f.file === "a.rs");
  assert.equal(x.consensus, 2, "consensus 累计 2");
  assert.equal(x.severity, "blocker", "取最高严重度 blocker");
  assert.equal(x.evidence, "longer evidence", "取最长证据");
  assert.deepEqual(x.sources, ["r1#0", "r1#1"], "sources 累计");
  assert.equal(out[0].severity, "blocker", "blocker 排在 major 前");
});

test("dedupeFindings: 空标题且空文件被丢弃 / 空输入", () => {
  assert.deepEqual(dedupeFindings([]), []);
  assert.deepEqual(dedupeFindings(null), []);
  assert.deepEqual(dedupeFindings([{ severity: "minor" }]), [], "无 title 无 file → 丢弃");
});

// ── tallyDecision:怀疑投票分流(accept/reject/pending + 末轮从严)──────────────
test("tallyDecision: 接受/拒绝/争议 三态 + 阈值边界", () => {
  // 默认 accept≥0.7, reject≤0.35
  assert.equal(tallyDecision(7, 3), "accept", "0.7 命中接受阈值(含等号)");
  assert.equal(tallyDecision(8, 2), "accept", "0.8 接受");
  assert.equal(tallyDecision(3, 7), "reject", "0.3 ≤ 0.35 拒绝");
  assert.equal(tallyDecision(35, 65), "reject", "0.35 命中拒绝阈值(含等号)");
  assert.equal(tallyDecision(5, 5), "pending", "0.5 争议 → pending");
  assert.equal(tallyDecision(6, 4), "pending", "0.6 在 0.35~0.7 之间 → pending");
});

test("tallyDecision: 无票 → 非末轮 pending / 末轮 reject", () => {
  assert.equal(tallyDecision(0, 0), "pending", "无票非末轮等下轮");
  assert.equal(tallyDecision(0, 0, { isFinalRound: true }), "reject", "无票末轮从严拒");
});

test("tallyDecision: 末轮争议从严 reject(不放水)", () => {
  assert.equal(tallyDecision(5, 5, { isFinalRound: true }), "reject", "末轮 0.5 争议 → reject");
  assert.equal(tallyDecision(6, 4, { isFinalRound: true }), "reject", "末轮 0.6 未达接受 → reject");
  assert.equal(tallyDecision(7, 3, { isFinalRound: true }), "accept", "末轮 0.7 仍接受");
});

test("tallyDecision: 自定义阈值", () => {
  assert.equal(tallyDecision(6, 4, { acceptRatio: 0.6 }), "accept", "降低接受阈值后 0.6 接受");
  assert.equal(tallyDecision(5, 5, { rejectRatio: 0.5 }), "reject", "提高拒绝阈值后 0.5 拒绝");
});

// ── extractJSON:围栏 / 平衡括号 / 垃圾 ───────────────────────────────────────
test("extractJSON: 纯 JSON / ```json 围栏 / 前后废话 / 嵌套 / 非法", () => {
  assert.deepEqual(extractJSON('{"a":1}'), { a: 1 });
  assert.deepEqual(extractJSON('```json\n{"a":1}\n```'), { a: 1 }, "去围栏");
  assert.deepEqual(extractJSON('废话{"a":[1,2]}尾巴'), { a: [1, 2] }, "前后废话 + 平衡扫描");
  assert.deepEqual(extractJSON('{"a":{"b":2}}'), { a: { b: 2 } }, "嵌套");
  assert.deepEqual(extractJSON('[{"id":1}]'), [{ id: 1 }], "数组");
  assert.equal(extractJSON("no json here"), null);
  assert.equal(extractJSON(""), null);
  assert.equal(extractJSON(null), null);
  assert.deepEqual(extractJSON('{"s":"含 } 的字符串"}'), { s: "含 } 的字符串" }, "字符串内括号不误判");
});

// ── parseMember:成员输出归一 ─────────────────────────────────────────────────
test("parseMember: 完整 / 缺字段 / 字符串 uncertain / 裸数组 findings", () => {
  const full = parseMember(
    '{"confidence":85,"findings":[{"title":"x","file":"a.rs"}],"uncertain_dimensions":[{"dimension":"c"}],"files_needed":["a.rs"]}',
  );
  assert.equal(full.confidence, 85);
  assert.equal(full.findings.length, 1);
  assert.equal(full.uncertain[0].dimension, "c");
  assert.deepEqual(full.filesNeeded, ["a.rs"]);

  const bare = parseMember("{}");
  assert.equal(bare.confidence, null, "缺 confidence → null");
  assert.deepEqual(bare.findings, []);
  assert.deepEqual(bare.uncertain, []);
  assert.deepEqual(bare.filesNeeded, []);

  const strU = parseMember('{"uncertain_dimensions":["守恒"]}');
  assert.equal(strU.uncertain[0].dimension, "守恒", "字符串 uncertain 转对象");

  const arr = parseMember('[{"title":"x","file":"a.rs"}]');
  assert.equal(arr.findings.length, 1, "裸数组当 findings");

  assert.deepEqual(parseMember("garbage").findings, [], "无法解析 → 安全默认");
});

// ── isSafeRepoPath:路径安全 ──────────────────────────────────────────────────
test("isSafeRepoPath: 合法相对路径 / 拒绝穿越/绝对/空字节", () => {
  assert.equal(isSafeRepoPath("server/src/fauna/mimic_spider.rs"), true);
  assert.equal(isSafeRepoPath("a-b/c_d.rs"), true);
  assert.equal(isSafeRepoPath("/etc/passwd"), false, "绝对路径拒");
  assert.equal(isSafeRepoPath("../../etc/passwd"), false, ".. 穿越拒");
  assert.equal(isSafeRepoPath("a/../b"), false, "中间 .. 拒");
  assert.equal(isSafeRepoPath(""), false, "空拒");
  assert.equal(isSafeRepoPath("a\0b"), false, "空字节拒");
  assert.equal(isSafeRepoPath("a b.rs"), false, "空格(非白名单字符)拒");
});

// ── expandModelList:力大砖飞展开 ─────────────────────────────────────────────
test("expandModelList: *count 展开 / 混合 / 空", () => {
  assert.deepEqual(expandModelList("a*3"), ["a", "a", "a"]);
  assert.deepEqual(expandModelList("a*2, b, c*1"), ["a", "a", "b", "c"]);
  assert.deepEqual(expandModelList("deepseek-v4-flash, sensenova-6.7-flash-lite"), [
    "deepseek-v4-flash",
    "sensenova-6.7-flash-lite",
  ]);
  assert.deepEqual(expandModelList(""), []);
  assert.deepEqual(expandModelList("a*0"), [], "*0 展开为空");
});

// ── pickTier / buildTierPanel:动态分档 ───────────────────────────────────────
test("pickTier: 按行数选档 + 文件数升档 + 末档兜底", () => {
  assert.equal(pickTier(10, 1).label, "trivial", "10 行 → trivial");
  assert.equal(pickTier(200, 3).label, "small", "200 行 → small");
  assert.equal(pickTier(500, 3).label, "medium", "500 行 → medium");
  assert.equal(pickTier(99999, 50).label, "huge", "超大 → huge 兜底");
  // 文件数 ≥ 15 升一档:40 行本是 trivial,30 文件 → small
  assert.equal(pickTier(40, 30).label, "small", "改动面广升一档");
  assert.equal(pickTier(99999, 999).label, "huge", "已是末档不再升");
});

test("buildTierPanel: 按 flash/lite 数量展开 finder 面板 / lite=0 略过", () => {
  const trivial = TIERS.find((t) => t.label === "trivial");
  const panel = buildTierPanel(trivial, { flashModel: "f", liteModel: "l" });
  assert.deepEqual(panel, ["f", "f"], "trivial: flash×2,lite=0 略过");
  const medium = TIERS.find((t) => t.label === "medium");
  const mp = buildTierPanel(medium, { flashModel: "f", liteModel: "l" });
  assert.equal(mp.filter((m) => m === "f").length, 4, "medium flash×4");
  assert.equal(mp.filter((m) => m === "l").length, 2, "medium lite×2");
});

test("TIERS: maxLines 升序 + 末档 Infinity 兜底", () => {
  for (let i = 1; i < TIERS.length; i++) {
    assert.ok(TIERS[i].maxLines > TIERS[i - 1].maxLines, `第 ${i} 档 maxLines 必须严格升序`);
  }
  assert.equal(TIERS[TIERS.length - 1].maxLines, Infinity, "末档 Infinity 兜底");
});

// ── dedupeUncertain:弱项归一合并 ─────────────────────────────────────────────
test("dedupeUncertain: 同维度合并 reason/need / 大小写归一 / 丢空", () => {
  const out = dedupeUncertain([
    { dimension: "守恒", reason: "r1", need: "看 ledger" },
    { dimension: "守恒", reason: "r2", need: "看 ledger" },
    { dimension: "测试" },
    { reason: "无维度" },
  ]);
  assert.equal(out.length, 2, "守恒合并 + 测试,无维度丢弃");
  const sh = out.find((u) => u.dimension === "守恒");
  assert.match(sh.reason, /r1/);
  assert.match(sh.reason, /r2/, "reason 合并");
  assert.ok(!sh.need.includes("看 ledger看 ledger"), "重复 need 不叠加");
});
