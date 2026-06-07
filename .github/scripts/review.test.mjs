// Review v2 纯逻辑测试 —— `node --test .github/scripts/review.test.mjs`
// 锁住 SDK orchestrator 外壳的确定性部分:diff 规模分档(pickTier)、finder 预算(tierBudget)、
// JSON 解析容错(extractJSON)、token 聚合(sumModelUsage)。
// agent 编排本身(opus 自主 spawn flash swarm + 玩法)非确定性,由 dry-run / 真实 /review e2e 验证。

import { test } from "node:test";
import assert from "node:assert/strict";
import { pickTier, tierBudget, extractJSON, sumModelUsage, TIERS } from "./review.mjs";

// ── pickTier:按行数分档 + 文件数升档 + 末档兜底 ────────────────────────────────
test("pickTier: 行数定档 / 文件数升档 / 末档兜底", () => {
  assert.equal(pickTier(10, 1).label, "trivial", "10 行 → trivial");
  assert.equal(pickTier(40, 1).label, "trivial", "40 行边界 → trivial");
  assert.equal(pickTier(41, 1).label, "small", "41 行 → small");
  assert.equal(pickTier(200, 3).label, "small", "200 行 → small");
  assert.equal(pickTier(500, 3).label, "medium", "500 行 → medium");
  assert.equal(pickTier(2000, 3).label, "large", "2000 行 → large");
  assert.equal(pickTier(99999, 50).label, "huge", "超大 → huge 兜底");
  assert.equal(pickTier(40, 30).label, "small", "改动面广(30 文件)升一档:trivial→small");
  assert.equal(pickTier(99999, 999).label, "huge", "已是末档不再升");
  assert.equal(pickTier(0, 0).label, "trivial", "空 diff → trivial");
});

// ── TIERS:结构不变式 ──────────────────────────────────────────────────────────
test("TIERS: maxLines 升序 + 末档 Infinity + finders 合法且递增不超 10", () => {
  for (let i = 1; i < TIERS.length; i++) {
    assert.ok(TIERS[i].maxLines > TIERS[i - 1].maxLines, `第 ${i} 档 maxLines 必须严格升序`);
    assert.ok(TIERS[i].finders >= TIERS[i - 1].finders, `第 ${i} 档 finders 不应递减`);
  }
  assert.equal(TIERS[TIERS.length - 1].maxLines, Infinity, "末档 Infinity 兜底");
  for (const t of TIERS) {
    assert.ok(t.finders >= 1 && t.finders <= 10, `${t.label} finders 应在 1~10(SDK 并发上限)`);
  }
});

// ── tierBudget:finder 预算(默认/pin/cap)────────────────────────────────────────
test("tierBudget: 取档位 finders / pin 覆盖 / 封顶 10 / 下限 1", () => {
  assert.equal(tierBudget({ finders: 6 }), 6, "默认取档位 finders");
  assert.equal(tierBudget({ finders: 6 }, { pin: 2 }), 2, "pin 覆盖档位");
  assert.equal(tierBudget({ finders: 8 }, { pin: 99 }), 10, "pin 超 cap 封顶 10");
  assert.equal(tierBudget({ finders: 99 }), 10, "档位超 cap 封顶 10");
  assert.equal(tierBudget({ finders: 0 }), 3, "finders:0 falsy → 走代码默认 3");
  assert.equal(tierBudget({ finders: 3 }, { cap: 0 }), 1, "cap 0 → max(1) 下限兜底");
  assert.equal(tierBudget(null, { pin: null }), 3, "无 tier + 无 pin → 代码默认 3");
  assert.equal(tierBudget({ finders: 8 }, { cap: 4 }), 4, "自定义 cap");
});

// ── extractJSON:围栏 / 平衡括号 / 垃圾 / 字符串内括号 ──────────────────────────
test("extractJSON: 纯 JSON / ```json 围栏 / 前后废话 / 嵌套 / 数组 / 非法 / 串内括号", () => {
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

// ── sumModelUsage:token 聚合(cache 计入 input) + 降序 ──────────────────────────
test("sumModelUsage: 聚合 input/output(含 cache)/total / 按 total 降序 / 空", () => {
  const rows = sumModelUsage({
    "deepseek-v4-flash": { inputTokens: 100, outputTokens: 20, cacheReadInputTokens: 50, costUSD: 0.01 },
    "deepseek-v4-pro": { inputTokens: 1000, outputTokens: 300, cacheCreationInputTokens: 200, costUSD: 0.05 },
  });
  assert.equal(rows.length, 2);
  assert.equal(rows[0].model, "deepseek-v4-pro", "pro total 更大,排第一");
  assert.equal(rows[0].input, 1200, "pro input = 1000 + cacheCreation 200");
  assert.equal(rows[0].output, 300);
  assert.equal(rows[0].total, 1500);
  assert.equal(rows[1].input, 150, "flash input = 100 + cacheRead 50");
  assert.equal(rows[1].total, 170);
  assert.deepEqual(sumModelUsage({}), [], "空 → []");
  assert.deepEqual(sumModelUsage(null), [], "null → []");
  assert.equal(sumModelUsage({ m: {} })[0].total, 0, "缺字段 → 0");
});
