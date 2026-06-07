#!/usr/bin/env node
// pi-bench —— LLM 代码审核能力 benchmark。
//
// 一组候选模型用【相同基础配置】审同一个 PR;把所有人的 findings 汇总去重,交一个固定 jury
// 逐条判定是否「有效错误」(默认怀疑,多数 REAL 才算数);再算每个模型平均找到几条有效错误
// (同模型多实例取平均)+ 精确率 + 独有贡献。产出自包含 HTML 可视化(内联 SVG,无外部依赖)。
//
// 自包含:Node 内置 fetch + gh,只从 review.mjs 复用纯 extractJSON。全 OpenAI 兼容端点。
// key/模型/PR/实例数全 env 可覆盖,key 绝不写进文件(运行时 env 注入)。
//
// 用法:
//   PI_CLIPROXY_KEY=.. PI_DEEPSEEK_KEY=.. PI_OLLAMA_KEY=.. PI_ALIYUN_KEY=.. PI_GEMINI_KEY=.. \
//   BENCH_PR=411 BENCH_INSTANCES=3 node scripts/llm-bench/pi-bench.mjs
//   → 写 scripts/llm-bench/report.html + report.json

import { execSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { extractJSON } from "../../.github/scripts/review.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));

// ── provider 注册表(全 OpenAI 兼容;key 从 env)──────────────────────────────
const PROVIDERS = {
  cliproxy: { baseUrl: process.env.PI_CLIPROXY_BASE_URL || "https://proxy.kizun4.uk/v1", key: process.env.PI_CLIPROXY_KEY },
  deepseek: { baseUrl: process.env.PI_DEEPSEEK_BASE_URL || "https://api.deepseek.com", key: process.env.PI_DEEPSEEK_KEY },
  ollama: { baseUrl: process.env.PI_OLLAMA_BASE_URL || "https://ollama.com/v1", key: process.env.PI_OLLAMA_KEY },
  aliyun: { baseUrl: process.env.PI_ALIYUN_BASE_URL || "https://ws-h0zvje40wib8deor.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1", key: process.env.PI_ALIYUN_KEY },
  gemini: { baseUrl: process.env.PI_GEMINI_BASE_URL || "https://ai.hybgzs.com/v1", key: process.env.PI_GEMINI_KEY },
  muyuan: { baseUrl: process.env.PI_MUYUAN_BASE_URL || "https://muyuan.do/v1", key: process.env.PI_MUYUAN_KEY }, // newapi 代理:gpt-5.x 全家
};

// ── 候选 + jury(env 逗号分隔覆盖)─────────────────────────────────────────────
const CANDIDATES = (process.env.BENCH_MODELS ||
  [
    "cliproxy/deepseek-v4-flash",
    "cliproxy/sensenova-6.7-flash-lite",
    "deepseek/deepseek-v4-pro",
    "ollama/minimax-m3",
    "aliyun/qwen3.7-max",
    "aliyun/qwen3.7-plus",
    "gemini/gemini-3.5-flash",
    "muyuan/gpt-5.5",
    "muyuan/gpt-5.4-mini",
  ].join(",")
)
  .split(",").map((s) => s.trim()).filter(Boolean);

// jury 用跨家强模型,降低单家偏置(候选与 jury 可重叠,recall 度量不受影响)
// jury 用可靠强模型(gemini 经 hybgzs 高并发会 429,不入 jury,只当候选)
const JURY = (process.env.BENCH_JURY || "deepseek/deepseek-v4-pro,aliyun/qwen3.7-max,aliyun/qwen3.7-plus")
  .split(",").map((s) => s.trim()).filter(Boolean);

const INSTANCES = Math.max(1, parseInt(process.env.BENCH_INSTANCES || "3", 10));
const PR = process.env.BENCH_PR || "411";
const MAX_DIFF = parseInt(process.env.BENCH_MAX_DIFF || "30000", 10);
const CONCURRENCY = Math.max(1, parseInt(process.env.BENCH_CONCURRENCY || "8", 10));
const TIMEOUT_MS = Math.max(30_000, parseInt(process.env.BENCH_TIMEOUT_MS || "180000", 10));
const JURY_ACCEPT = parseFloat(process.env.BENCH_JURY_ACCEPT || "0.5"); // ≥ 此比例 REAL 即判有效

const usage = {}; // model -> {total, calls}

// ── 工具 ─────────────────────────────────────────────────────────────────────
const gh = (a) => execSync(`gh ${a}`, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function mapLimit(items, limit, fn) {
  const out = new Array(items.length);
  let i = 0;
  const w = async () => { while (i < items.length) { const k = i++; out[k] = await fn(items[k], k); } };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, w));
  return out;
}

function parseModel(full) {
  const s = String(full).trim();
  const i = s.indexOf("/");
  return i < 0 ? { provider: null, id: s } : { provider: s.slice(0, i), id: s.slice(i + 1) };
}

async function chat(model, content, { label = "", retries = 3 } = {}) {
  const { provider, id } = parseModel(model);
  const p = PROVIDERS[provider];
  if (!p || !p.key) { console.error(`[${label}] ${model}: provider ${provider} 无 key,跳过`); return null; }
  const base = p.baseUrl.replace(/\/+$/, "");
  for (let a = 1; a <= retries; a++) {
    const ctrl = new AbortController();
    const t = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
    try {
      const res = await fetch(`${base}/chat/completions`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${p.key}` },
        body: JSON.stringify({ model: id, messages: [{ role: "user", content }] }),
        signal: ctrl.signal,
      });
      clearTimeout(t);
      if (!res.ok) throw new Error(`HTTP ${res.status} ${(await res.text()).slice(0, 160)}`);
      const d = await res.json();
      const u = d?.usage; if (u) { const m = usage[model] || (usage[model] = { total: 0, calls: 0 }); m.total += u.total_tokens || (u.prompt_tokens || 0) + (u.completion_tokens || 0); m.calls++; }
      const text = d?.choices?.[0]?.message?.content;
      if (!text || !text.trim()) throw new Error("空响应");
      return text;
    } catch (e) {
      clearTimeout(t);
      console.error(`[${label}] ${model} ${a}/${retries} 失败: ${e.message}`);
      if (a === retries) return null;
      await sleep(1500 * a);
    }
  }
  return null;
}

// ── finding 归一 key:同 file|line|标题 视为同一条 ──────────────────────────────
const keyOf = (f) => `${String(f.file || "").trim()}|${String(f.line ?? "").trim()}|${String(f.title || "").trim().toLowerCase().replace(/\s+/g, " ")}`;
const sevRank = (s) => ({ blocker: 0, major: 1, minor: 2 }[String(s || "").toLowerCase()] ?? 3);

const GUIDE = `Bong = 末法残土修仙 MC 沙盒,三层 server(Rust/Bevy)+ agent(TS)+ client(Java)。审核要点:
逻辑/边界 off-by-one/并发/错误分支;世界观(六境界醒灵→引气→凝脉→固元→通灵→化虚、禁筑基金丹元婴;骨币货币/灵石燃料;
灵气守恒律真元流动必须走 qi_physics::QiTransfer 不得凭空增减;物理常数唯一源不得硬编);IPC schema 双端对齐;测试饱和。`;

const reviewPrompt = (diff) =>
  `你是 Bong 项目 PR 审核员。${GUIDE}\n\n审下面 diff,只找**真问题**,每条带 file:line + 证据,没把握不报,没问题就空数组。` +
  `只输出纯 JSON(无解释):{"findings":[{"severity":"blocker|major|minor","file":"路径","line":"行号","title":"一句话问题","why":"为什么"}]}\n\n` +
  `## diff\n\`\`\`diff\n${diff}\n\`\`\``;

const juryPrompt = (diff, f) =>
  `你是 PR 审核**怀疑型裁判**。判断下面这条 finding 是不是**真问题**:能在 diff 定位、逻辑成立、周围代码确实没处理。` +
  `默认怀疑,证据不足 / 推测 / 风格洁癖一律 NOT_REAL。${GUIDE}\n\n` +
  `## diff\n\`\`\`diff\n${diff}\n\`\`\`\n\n## 待裁定 finding\n[${f.severity}] ${f.file}:${f.line} — ${f.title}(${f.why || ""})\n\n` +
  `只输出 JSON:{"verdict":"REAL|NOT_REAL","reason":"一句话"}`;

function toFindings(raw) {
  const j = extractJSON(raw);
  const arr = Array.isArray(j) ? j : Array.isArray(j?.findings) ? j.findings : [];
  return arr.filter((f) => f && (f.title || f.file)).map((f) => ({
    severity: String(f.severity || "minor").toLowerCase(),
    file: String(f.file || "").trim(),
    line: String(f.line ?? "").trim(),
    title: String(f.title || "").trim(),
    why: String(f.why || "").trim(),
  }));
}

// ── 主流程 ───────────────────────────────────────────────────────────────────
async function main() {
  const live = Object.entries(PROVIDERS).filter(([, p]) => p.key).map(([n]) => n);
  console.error(`pi-bench: PR #${PR} · ${CANDIDATES.length} 模型 ×${INSTANCES} 实例 · jury ${JURY.length} · 在线 provider [${live.join(", ")}]`);

  let diff = gh(`pr diff ${PR}`);
  let truncated = false;
  if (diff.length > MAX_DIFF) { diff = diff.slice(0, MAX_DIFF); truncated = true; }
  const meta = JSON.parse(gh(`pr view ${PR} --json title`));

  // ① 每个候选 × 每个实例 跑一次 review
  const jobs = [];
  for (const model of CANDIDATES) for (let inst = 0; inst < INSTANCES; inst++) jobs.push({ model, inst });
  console.error(`▶ 审核:${jobs.length} 次调用(并发 ${CONCURRENCY})`);
  const runs = await mapLimit(jobs, CONCURRENCY, (j) =>
    chat(j.model, reviewPrompt(diff), { label: `review:${j.model}#${j.inst}` }).then((r) => ({ ...j, findings: r ? toFindings(r) : null })),
  );

  // ② 汇总去重(跨所有模型/实例)
  const pool = new Map(); // key -> {...finding, byModels:Set}
  for (const r of runs) {
    if (!r.findings) continue;
    for (const f of r.findings) {
      const k = keyOf(f);
      const e = pool.get(k);
      if (!e) pool.set(k, { ...f, key: k, byModels: new Set([r.model]) });
      else { e.byModels.add(r.model); if (sevRank(f.severity) < sevRank(e.severity)) e.severity = f.severity; if ((f.why || "").length > (e.why || "").length) e.why = f.why; }
    }
  }
  const unique = [...pool.values()];
  console.error(`  汇总 ${unique.length} 条去重 finding,交 jury 裁定`);

  // ③ jury 逐条裁定有效性
  const verdicts = await mapLimit(unique, CONCURRENCY, async (f) => {
    const votes = await Promise.all(JURY.map((jm) => chat(jm, juryPrompt(diff, f), { label: `jury:${jm}` }).then((r) => {
      const v = extractJSON(r);
      return v && String(v.verdict || "").toUpperCase().includes("REAL") && !String(v.verdict).toUpperCase().includes("NOT") ? 1 : 0;
    })));
    const real = votes.reduce((a, b) => a + b, 0);
    const valid = JURY.length ? real / JURY.length >= JURY_ACCEPT : false;
    return { key: f.key, real, of: JURY.length, valid };
  });
  const validKeys = new Set(verdicts.filter((v) => v.valid).map((v) => v.key));
  console.error(`  jury 判定有效 ${validKeys.size}/${unique.length}`);

  // ④ 每模型打分:平均(每实例报告数 / 命中有效数),精确率,独有有效贡献
  const byModel = {};
  for (const model of CANDIDATES) byModel[model] = { runs: [], failed: 0 };
  for (const r of runs) {
    if (!r.findings) { byModel[r.model].failed++; continue; }
    const total = r.findings.length;
    const valid = r.findings.filter((f) => validKeys.has(keyOf(f))).length;
    byModel[r.model].runs.push({ total, valid });
  }
  // 独有有效:某有效 finding 只被这一个模型(任意实例)报过
  const validOwners = {};
  for (const f of unique) if (validKeys.has(f.key)) validOwners[f.key] = f.byModels;
  const results = CANDIDATES.map((model) => {
    const d = byModel[model];
    const n = d.runs.length || 1;
    const avgTotal = d.runs.reduce((a, r) => a + r.total, 0) / n;
    const avgValid = d.runs.reduce((a, r) => a + r.valid, 0) / n;
    const soleValid = Object.values(validOwners).filter((owners) => owners.size === 1 && owners.has(model)).length;
    return {
      model,
      instances: d.runs.length,
      failed: d.failed,
      avgTotal: +avgTotal.toFixed(2),
      avgValid: +avgValid.toFixed(2),
      precision: avgTotal ? +(avgValid / avgTotal).toFixed(2) : 0,
      soleValid,
      tokens: usage[model]?.total || 0,
    };
  }).sort((a, b) => b.avgValid - a.avgValid);

  const summary = { pr: PR, title: meta.title, truncated, validTotal: validKeys.size, uniqueTotal: unique.length, instances: INSTANCES, jury: JURY, results };
  writeFileSync(join(HERE, "report.json"), JSON.stringify(summary, null, 2));
  writeFileSync(join(HERE, "report.html"), renderHTML(summary));

  // 控制台表
  console.error("\n=== 结果(按平均有效错误数排序)===");
  console.error("模型".padEnd(36) + "实例  平均有效  平均报告  精确率  独有有效  tokens");
  for (const r of results) {
    console.error(
      r.model.padEnd(36) +
      `${r.instances}/${INSTANCES}`.padEnd(6) +
      String(r.avgValid).padEnd(10) + String(r.avgTotal).padEnd(10) +
      String(r.precision).padEnd(8) + String(r.soleValid).padEnd(10) + r.tokens,
    );
  }
  console.error(`\n✅ 报告:${join(HERE, "report.html")}(+ report.json)`);
}

// ── 自包含 HTML(内联 SVG 横向柱状图 + 表)──────────────────────────────────────
function renderHTML(s) {
  const max = Math.max(0.01, ...s.results.map((r) => r.avgValid));
  const bar = (r, i) => {
    const w = Math.round((r.avgValid / max) * 560);
    const hue = 200 - Math.round((r.avgValid / max) * 90); // 多→偏绿
    return `<g transform="translate(0,${i * 30})">
      <text x="270" y="15" text-anchor="end" font-size="12" fill="#ddd">${esc(r.model)}</text>
      <rect x="280" y="3" width="${w}" height="18" rx="3" fill="hsl(${hue},65%,50%)"/>
      <text x="${288 + w}" y="16" font-size="12" fill="#9fb">${r.avgValid}</text>
    </g>`;
  };
  const rows = s.results.map((r) => `<tr>
    <td>${esc(r.model)}</td><td>${r.instances}/${s.instances}${r.failed ? ` <span class=f>(${r.failed}失败)</span>` : ""}</td>
    <td class=n><b>${r.avgValid}</b></td><td class=n>${r.avgTotal}</td><td class=n>${(r.precision * 100).toFixed(0)}%</td>
    <td class=n>${r.soleValid}</td><td class=n>${r.tokens.toLocaleString()}</td></tr>`).join("");
  return `<!doctype html><html lang=zh><head><meta charset=utf-8><title>pi-bench · PR #${s.pr}</title>
<style>body{background:#15171c;color:#e6e6e6;font:14px/1.5 system-ui,sans-serif;max-width:980px;margin:24px auto;padding:0 16px}
h1{font-size:20px}.sub{color:#9aa;font-size:13px;margin-bottom:18px}
table{border-collapse:collapse;width:100%;margin-top:18px}th,td{padding:7px 10px;border-bottom:1px solid #2a2e36;text-align:left}
th{color:#9aa;font-weight:600;font-size:12px}.n{text-align:right;font-variant-numeric:tabular-nums}.f{color:#e88}
td b{color:#7fdca0}.note{color:#9aa;font-size:12px;margin-top:14px}</style></head><body>
<h1>🧪 pi-bench · 模型代码审核能力</h1>
<div class=sub>PR #${s.pr}「${esc(s.title || "")}」· 每模型 ${s.instances} 实例取平均 · jury(${s.jury.map(esc).join(", ")})判定有效错误 ${s.validTotal}/${s.uniqueTotal} 条${s.truncated ? " · diff 已截断" : ""}</div>
<svg width="700" height="${s.results.length * 30 + 10}" font-family="system-ui">${s.results.map(bar).join("")}</svg>
<table><thead><tr><th>模型</th><th>实例</th><th>平均有效错误</th><th>平均报告</th><th>精确率</th><th>独有有效</th><th>tokens</th></tr></thead><tbody>${rows}</tbody></table>
<div class=note>「有效错误」= 该模型报告且被 jury 多数判定为真问题的 finding 数(同模型多实例取平均)。「精确率」= 有效/报告。「独有有效」= 仅此模型发现、其他模型都漏的有效问题数。jury 成员与候选可能重叠,自评略有偏置。</div>
</body></html>`;
}
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

main().catch((e) => { console.error(e); process.exit(1); });
