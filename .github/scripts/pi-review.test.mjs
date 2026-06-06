// Pi 轮控 reviewer 纯逻辑测试 —— `node --test .github/scripts/pi-review.test.mjs`
// 锁住决策行为:倒计时分档(语气逐级加码)、自信度聚合(收敛门控判据)、
// findings 去重(consensus/严重度合并)、JSON/成员解析容错、补充文件路径安全过滤。
// 这些是 B 方案"确定性外壳"的核心——任何回归(门控阈值错算、倒计时跳档、去重漏合并)都要立刻撞红。

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  normalizeConfidence,
  aggregateConfidence,
  computeCountdown,
  dedupeFindings,
  dedupeUncertain,
  extractJSON,
  parseMember,
  isSafeRepoPath,
  fetchFiles,
  expandModelList,
  parseModel,
  pickTier,
  buildTierPanel,
  TIERS,
} from "./pi-review.mjs";

// ── normalizeConfidence ──────────────────────────────────────────────────────
test("normalizeConfidence: 整数/字符串/边界/越界/非法", () => {
  assert.equal(normalizeConfidence(80), 80, "整数原样");
  assert.equal(normalizeConfidence("75"), 75, "数字字符串可解析");
  assert.equal(normalizeConfidence(0), 0, "下边界 0 合法");
  assert.equal(normalizeConfidence(100), 100, "上边界 100 合法");
  assert.equal(normalizeConfidence(150), 100, "越上界钳到 100");
  assert.equal(normalizeConfidence(-5), 0, "越下界钳到 0");
  assert.equal(normalizeConfidence(82.6), 83, "四舍五入");
  assert.equal(normalizeConfidence(NaN), null, "NaN → fallback(null)");
  assert.equal(normalizeConfidence(undefined), null, "undefined → null");
  assert.equal(normalizeConfidence("abc"), null, "非数字串 → null");
  assert.equal(normalizeConfidence(undefined, 50), 50, "fallback 可指定");
});

// ── aggregateConfidence(收敛门控判据)─────────────────────────────────────────
test("aggregateConfidence: 空 / 单值 / 奇偶中位数 / 过滤非法 / 全非法", () => {
  assert.deepEqual(aggregateConfidence([]), { median: null, min: null, max: null, n: 0 }, "空 → 全 null");
  assert.deepEqual(aggregateConfidence([88]), { median: 88, min: 88, max: 88, n: 1 }, "单值");
  assert.deepEqual(aggregateConfidence([60, 90, 70]), { median: 70, min: 60, max: 90, n: 3 }, "奇数取中位");
  assert.deepEqual(
    aggregateConfidence([60, 80, 70, 90]),
    { median: 75, min: 60, max: 90, n: 4 },
    "偶数取中间两数均值(四舍五入)",
  );
  assert.deepEqual(
    aggregateConfidence([null, "bad", 80, undefined, 60]),
    { median: 70, min: 60, max: 80, n: 2 },
    "非法值被剔除,只对有效值聚合",
  );
  assert.deepEqual(aggregateConfidence([null, "x", NaN]), { median: null, min: null, max: null, n: 0 }, "全非法 → null");
});

test("aggregateConfidence: 离群值不被单点拉偏(用中位数而非均值)", () => {
  // 两个低 + 一个 100:均值=66 会误判收敛;中位数=30 正确反映"多数没把握"
  const agg = aggregateConfidence([20, 30, 100]);
  assert.equal(agg.median, 30, "中位数=30,不被 100 拉偏");
});

// ── computeCountdown(倒计时 + 语气逐级加码)──────────────────────────────────
test("computeCountdown: 唯一一轮 = 立即硬上限", () => {
  const cd = computeCountdown(1, 1);
  assert.equal(cd.left, 0);
  assert.match(cd.label, /唯一一轮/);
  assert.match(cd.text, /硬上限/);
});

test("computeCountdown: 首轮从容、中段报余量、倒数第二轮警告、末轮硬收口", () => {
  const r1 = computeCountdown(1, 4);
  assert.equal(r1.left, 3);
  assert.match(r1.label, /第 1\/4 轮/);
  assert.match(r1.text, /从容/);

  const r2 = computeCountdown(2, 4); // left=2
  assert.equal(r2.left, 2);
  assert.match(r2.text, /还剩 2 轮/);

  const r3 = computeCountdown(3, 4); // left=1 → 警告
  assert.equal(r3.left, 1);
  assert.match(r3.label, /只剩 1 轮/);
  assert.match(r3.text, /⚠️/);

  const r4 = computeCountdown(4, 4); // left=0 → 硬收口
  assert.equal(r4.left, 0);
  assert.match(r4.label, /最终轮/);
  assert.match(r4.text, /强制收口/);
});

test("computeCountdown: MAX_ROUNDS=2 两轮各自命中首轮/末轮分支(无跳档)", () => {
  const a = computeCountdown(1, 2);
  assert.match(a.label, /第 1\/2 轮/, "第一轮走从容分支");
  const b = computeCountdown(2, 2);
  assert.match(b.label, /最终轮/, "第二轮直接末轮硬收口(left=0)");
  assert.equal(b.left, 0);
});

// ── dedupeFindings(去重 + consensus + 严重度合并)────────────────────────────
test("dedupeFindings: 同 file:line:title 合并、累计 consensus、取最高严重度", () => {
  const merged = dedupeFindings([
    { severity: "minor", file: "a.rs", line: "10", title: "竞态", evidence: "x", why: "短", by: "r1#0" },
    { severity: "blocker", file: "a.rs", line: "10", title: "竞态", evidence: "更长的证据片段", why: "更长的原因说明", by: "r2#1" },
  ]);
  assert.equal(merged.length, 1, "合并为 1 条");
  assert.equal(merged[0].consensus, 2, "consensus 累计到 2");
  assert.equal(merged[0].severity, "blocker", "取最高严重度 blocker");
  assert.equal(merged[0].evidence, "更长的证据片段", "保留更长的证据");
  assert.equal(merged[0].why, "更长的原因说明", "保留更长的原因");
  assert.deepEqual(merged[0].sources, ["r1#0", "r2#1"], "记录所有来源");
});

test("dedupeFindings: 不同 file/line/title 各自独立,按严重度→consensus 排序", () => {
  const out = dedupeFindings([
    { severity: "minor", file: "b.rs", line: "1", title: "次要" },
    { severity: "blocker", file: "c.rs", line: "2", title: "阻塞" },
    { severity: "major", file: "d.rs", line: "3", title: "主要A" },
    { severity: "major", file: "d.rs", line: "3", title: "主要A" }, // 与上一条同键 → consensus 2
  ]);
  assert.equal(out.length, 3, "三个不同问题");
  assert.equal(out[0].title, "阻塞", "blocker 排最前");
  assert.equal(out[1].title, "主要A", "major 次之");
  assert.equal(out[1].consensus, 2, "major 那条 consensus=2");
  assert.equal(out[2].title, "次要", "minor 垫底");
});

test("dedupeFindings: 跳过空对象 / 无 title 且无 file 的噪声", () => {
  const out = dedupeFindings([null, {}, { why: "只有why" }, { file: "e.rs", line: "5", title: "真问题" }]);
  assert.equal(out.length, 1, "只留有效 finding");
  assert.equal(out[0].title, "真问题");
});

test("dedupeFindings: 空输入安全", () => {
  assert.deepEqual(dedupeFindings([]), []);
  assert.deepEqual(dedupeFindings(undefined), []);
});

// ── extractJSON(模型输出容错)────────────────────────────────────────────────
test("extractJSON: 纯 JSON / ```json 围栏 / 前后废话包裹 / 数组 / 嵌套", () => {
  assert.deepEqual(extractJSON('{"a":1}'), { a: 1 }, "纯对象");
  assert.deepEqual(extractJSON('```json\n{"a":2}\n```'), { a: 2 }, "json 围栏");
  assert.deepEqual(extractJSON("```\n[1,2,3]\n```"), [1, 2, 3], "裸围栏数组");
  assert.deepEqual(extractJSON('好的,结论如下:{"x":{"y":3}} 完毕'), { x: { y: 3 } }, "前后废话 + 嵌套");
  assert.deepEqual(extractJSON('[{"id":1},{"id":2}]'), [{ id: 1 }, { id: 2 }], "对象数组");
});

test("extractJSON: 字符串内的括号不误判闭合", () => {
  assert.deepEqual(extractJSON('{"s":"含 } 和 ] 的字符串"}'), { s: "含 } 和 ] 的字符串" }, "字符串内括号被跳过");
});

test("extractJSON: 无 JSON / 空 → null", () => {
  assert.equal(extractJSON("纯文本没有结构"), null);
  assert.equal(extractJSON(""), null);
  assert.equal(extractJSON(null), null);
});

// ── parseMember(成员输出归一)────────────────────────────────────────────────
test("parseMember: 完整结构原样解析", () => {
  const m = parseMember(
    JSON.stringify({
      confidence: 85,
      findings: [{ severity: "major", file: "a.rs", line: "1", title: "t", evidence: "e", why: "w" }],
      uncertain_dimensions: [{ dimension: "灵气守恒", reason: "没看到 zone 减", need: "qi_physics 调用处" }],
      files_needed: ["server/src/qi_physics/ledger.rs"],
    }),
  );
  assert.equal(m.confidence, 85);
  assert.equal(m.findings.length, 1);
  assert.equal(m.uncertain[0].dimension, "灵气守恒");
  assert.deepEqual(m.filesNeeded, ["server/src/qi_physics/ledger.rs"]);
});

test("parseMember: findings 为裸数组也接受", () => {
  const m = parseMember('[{"file":"a.rs","title":"t"}]');
  assert.equal(m.findings.length, 1, "裸数组当 findings");
  assert.equal(m.confidence, null, "无 confidence → null");
});

test("parseMember: 字符串型 uncertain_dimensions 归一成 {dimension}", () => {
  const m = parseMember(JSON.stringify({ uncertain_dimensions: ["测试饱和", "并发"] }));
  assert.deepEqual(
    m.uncertain.map((u) => u.dimension),
    ["测试饱和", "并发"],
  );
});

test("parseMember: 缺字段给安全默认 / 过滤空 finding / 解析失败全空", () => {
  const m = parseMember("{}");
  assert.deepEqual(m, { confidence: null, findings: [], uncertain: [], filesNeeded: [] }, "空对象 → 安全默认");

  const m2 = parseMember(JSON.stringify({ findings: [{}, { title: "有效" }, null] }));
  assert.equal(m2.findings.length, 1, "空/无 title&file 的 finding 被过滤");

  const m3 = parseMember("彻底解析不了");
  assert.deepEqual(m3, { confidence: null, findings: [], uncertain: [], filesNeeded: [] }, "解析失败 → 全空");
});

// ── isSafeRepoPath(补充文件路径安全)──────────────────────────────────────────
test("isSafeRepoPath: 接受相对仓库路径", () => {
  assert.equal(isSafeRepoPath("server/src/qi_physics/ledger.rs"), true);
  assert.equal(isSafeRepoPath("docs/plan-x-v1.md"), true);
  assert.equal(isSafeRepoPath("a.rs"), true);
});

test("isSafeRepoPath: 拒绝绝对路径 / .. 穿越 / 空 / 异常字符", () => {
  assert.equal(isSafeRepoPath("/etc/passwd"), false, "绝对路径");
  assert.equal(isSafeRepoPath("../../secret"), false, ".. 穿越");
  assert.equal(isSafeRepoPath("a/../../b"), false, "中段 ..");
  assert.equal(isSafeRepoPath(""), false, "空");
  assert.equal(isSafeRepoPath(null), false, "null");
  assert.equal(isSafeRepoPath("a b.rs"), false, "含空格(非白名单字符)");
  assert.equal(isSafeRepoPath("a;rm -rf.rs"), false, "含 shell 元字符");
});

// ── dedupeUncertain(弱项跨成员归并,驱动 fan-out 选维)─────────────────────────
test("dedupeUncertain: 同 dimension 归并、累计 reason/need、去重保序", () => {
  const out = dedupeUncertain([
    { dimension: "灵气守恒", reason: "没看到 zone 减", need: "qi_physics 调用处" },
    { dimension: "灵气守恒", reason: "招式只扣攻方", need: "环境扣减逻辑" },
    { dimension: "测试饱和", reason: "边界没覆盖" },
  ]);
  assert.equal(out.length, 2, "两个不同维度");
  assert.equal(out[0].dimension, "灵气守恒", "保持首次出现顺序");
  assert.match(out[0].reason, /没看到 zone 减/, "合并第一条 reason");
  assert.match(out[0].reason, /招式只扣攻方/, "合并第二条 reason");
  assert.match(out[0].need, /qi_physics 调用处/);
  assert.match(out[0].need, /环境扣减逻辑/);
});

test("dedupeUncertain: 大小写归一同维 / 跳过无 dimension / 重复 reason 不叠加", () => {
  const out = dedupeUncertain([
    { dimension: "并发", reason: "锁顺序" },
    { dimension: "并发", reason: "锁顺序" }, // 与上条同 reason → 不重复叠加
    { dimension: "并发 ", reason: "" }, // 带空格 + 空 reason
    null,
    { reason: "无 dimension 应跳过" },
  ]);
  assert.equal(out.length, 1, "并发(大小写/空格归一)合并为一条,噪声跳过");
  assert.equal(out[0].reason, "锁顺序", "相同 reason 不重复拼接");
});

test("dedupeUncertain: 空 / undefined 安全", () => {
  assert.deepEqual(dedupeUncertain([]), []);
  assert.deepEqual(dedupeUncertain(undefined), []);
});

// ── fetchFiles(补充上下文抓取:真实 IO + 去重 + 安全过滤 + 截断)────────────────
test("fetchFiles: 读相对文件、跨调用去重、跳不安全/不存在路径", () => {
  const dir = mkdtempSync(join(tmpdir(), "pi-fetch-"));
  const cwd0 = process.cwd();
  try {
    writeFileSync(join(dir, "a.txt"), "hello-A");
    writeFileSync(join(dir, "b.txt"), "hello-B");
    process.chdir(dir);
    const seen = new Set();
    const block = fetchFiles(
      ["a.txt", "a.txt", "../escape.txt", "/etc/passwd", "missing.txt", "b.txt"],
      seen,
    );
    assert.match(block, /### a\.txt/, "读到 a.txt");
    assert.match(block, /hello-A/, "含 a.txt 内容");
    assert.match(block, /### b\.txt/, "读到 b.txt");
    assert.match(block, /hello-B/, "含 b.txt 内容");
    assert.doesNotMatch(block, /escape|passwd|missing/, "不安全/不存在路径被跳过");
    assert.deepEqual([...seen].sort(), ["a.txt", "b.txt"], "已读集合只含成功读取的两个");
    assert.match(block, /补充代码上下文/, "带 main-版本 caveat 头");

    // 跨调用去重:already 已含 a.txt → 再请求 a.txt 被跳过 → 无新内容返回 ""
    assert.equal(fetchFiles(["a.txt"], seen), "", "已抓过的文件不再重复抓");
  } finally {
    process.chdir(cwd0);
    rmSync(dir, { recursive: true, force: true });
  }
});

test("fetchFiles: 无任何有效文件返回空串", () => {
  const seen = new Set();
  assert.equal(fetchFiles(["/abs/path", "../x", "nope.txt"], seen), "", "全无效 → 空串");
  assert.equal(seen.size, 0, "无文件被记入已读集合");
});

test("fetchFiles: 超长文件按 FILE_BYTES 截断并标注", () => {
  const dir = mkdtempSync(join(tmpdir(), "pi-fetch-big-"));
  const cwd0 = process.cwd();
  try {
    writeFileSync(join(dir, "big.txt"), "x".repeat(9000)); // > 默认 FILE_BYTES(8000)
    process.chdir(dir);
    const block = fetchFiles(["big.txt"], new Set());
    assert.match(block, /前 \d+ 字符/, "标注已截断");
    // 截断后正文长度应 ≈ FILE_BYTES(远小于 9000)
    const fenced = block.split("```")[1] || "";
    assert.ok(fenced.replace(/\n/g, "").length <= 8000, "正文被截到 FILE_BYTES 以内");
  } finally {
    process.chdir(cwd0);
    rmSync(dir, { recursive: true, force: true });
  }
});

// ── expandModelList(力大砖飞的 name*count 展开)────────────────────────────────
test("expandModelList: name*count 展开 + 保序 + provider 限定名", () => {
  assert.deepEqual(expandModelList("a*3, b, c*2"), ["a", "a", "a", "b", "c", "c"], "计数展开并保持顺序");
  assert.deepEqual(
    expandModelList("cliproxy/deepseek-v4-flash*2, deepseek/deepseek-v4-pro"),
    ["cliproxy/deepseek-v4-flash", "cliproxy/deepseek-v4-flash", "deepseek/deepseek-v4-pro"],
    "provider 限定名整体被当作模型名,只在末尾 *N 处展开",
  );
});

test("expandModelList: 空白容忍 / *0→至少1 / 空与 undefined", () => {
  assert.deepEqual(expandModelList("a * 3"), ["a", "a", "a"], "星号两侧空白容忍");
  assert.deepEqual(expandModelList("x*0"), ["x"], "*0 兜底至少 1 个");
  assert.deepEqual(expandModelList("  ,  , a ,"), ["a"], "空段被过滤、trim");
  assert.deepEqual(expandModelList(""), [], "空串 → 空数组");
  assert.deepEqual(expandModelList(undefined), [], "undefined → 空数组");
});

test("expandModelList: 力大砖飞配比总数正确", () => {
  const list = expandModelList(
    "cliproxy/deepseek-v4-flash*20,deepseek/deepseek-v4-pro*4,kiro/claude-sonnet-4-6*3,ollama/minimax-m3*3",
  );
  assert.equal(list.length, 30, "20+4+3+3 = 30 个面板成员");
  assert.equal(list.filter((m) => m === "deepseek/deepseek-v4-pro").length, 4, "pro 恰好 4 个");
  assert.equal(list.filter((m) => m === "cliproxy/deepseek-v4-flash").length, 20, "flash 恰好 20 个");
});

// ── parseModel(provider/id 路由解析)──────────────────────────────────────────
test("parseModel: 拆 provider/id / 无前缀 / 多段只切首个斜杠", () => {
  assert.deepEqual(parseModel("cliproxy/deepseek-v4-flash"), { provider: "cliproxy", id: "deepseek-v4-flash" });
  assert.deepEqual(parseModel("deepseek-v4-flash"), { provider: null, id: "deepseek-v4-flash" }, "无前缀 → provider null");
  assert.deepEqual(parseModel("kiro/claude-sonnet-4-6"), { provider: "kiro", id: "claude-sonnet-4-6" });
  assert.deepEqual(parseModel("a/b/c"), { provider: "a", id: "b/c" }, "只在首个 / 切分,id 保留余下斜杠");
  assert.deepEqual(parseModel("  "), { provider: null, id: "" }, "空白 → 空 id");
});

// ── pickTier(按改动规模动态分档)─────────────────────────────────────────────
test("pickTier: 按总变更行数选档(含每个边界 off-by-one)", () => {
  assert.equal(pickTier(0, 1).label, "trivial", "0 行 → trivial");
  assert.equal(pickTier(40, 1).label, "trivial", "40 行(上界)→ trivial");
  assert.equal(pickTier(41, 1).label, "small", "41 行 → small");
  assert.equal(pickTier(250, 1).label, "small", "250 行(上界)→ small");
  assert.equal(pickTier(251, 1).label, "medium", "251 行 → medium");
  assert.equal(pickTier(1000, 1).label, "medium", "1000 行(上界)→ medium");
  assert.equal(pickTier(1001, 1).label, "large", "1001 行 → large");
  assert.equal(pickTier(999999, 3).label, "large", "超大 → large 兜底");
});

test("pickTier: 文件数 ≥15 升一档,且封顶 large 不溢出", () => {
  assert.equal(pickTier(10, 14).label, "trivial", "14 文件不升档");
  assert.equal(pickTier(10, 15).label, "small", "15 文件:trivial→small 升一档");
  assert.equal(pickTier(200, 20).label, "medium", "small 行数 + 多文件 → medium");
  assert.equal(pickTier(5000, 50).label, "large", "已是 large + 多文件 → 仍 large(不溢出)");
});

test("pickTier: 非法输入安全(NaN/负数当 0 → trivial)", () => {
  assert.equal(pickTier(NaN, NaN).label, "trivial");
  assert.equal(pickTier(-100, 0).label, "trivial", "负行数当 0");
});

test("pickTier: 每档的配比单调不减(flash/pro/lite/rounds 越大档越多)", () => {
  for (let i = 1; i < TIERS.length; i++) {
    assert.ok(TIERS[i].flash >= TIERS[i - 1].flash, `${TIERS[i].label} flash ≥ 前档`);
    assert.ok(TIERS[i].pro >= TIERS[i - 1].pro, `${TIERS[i].label} pro ≥ 前档`);
    assert.ok(TIERS[i].rounds >= TIERS[i - 1].rounds, `${TIERS[i].label} rounds ≥ 前档`);
  }
  assert.equal(TIERS[0].pro, 0, "trivial 档 0 付费 pro(零成本)");
});

// ── buildTierPanel(档 → 角色模型展开)────────────────────────────────────────
test("buildTierPanel: 按角色模型 + 数量展开,pro=0 时不含 pro", () => {
  const roles = { flashModel: "cliproxy/flash", proModel: "deepseek/pro", liteModel: "cliproxy/lite" };
  const trivial = buildTierPanel({ flash: 4, pro: 0, lite: 2 }, roles);
  assert.equal(trivial.length, 6, "4+0+2 = 6");
  assert.ok(!trivial.includes("deepseek/pro"), "trivial 不含付费 pro");
  assert.equal(trivial.filter((m) => m === "cliproxy/flash").length, 4);

  const large = buildTierPanel({ flash: 20, pro: 4, lite: 8 }, roles);
  assert.equal(large.length, 32, "20+4+8 = 32 力大砖飞");
  assert.equal(large.filter((m) => m === "deepseek/pro").length, 4, "large 含 4 个 pro");
});
