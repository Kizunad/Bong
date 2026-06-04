#!/usr/bin/env node
// Hive-Think PR Reviewer —— "hive-think" 特殊模型 = N 个成员并行 debate(可混多个模型,同台对峙)。
//
// 发现 → 多轮严格裁定 → top-N 总裁决(少而准):
//   1) 发现:SWARM 个 finders 各包一个维度(模型在 HIVE_MODELS 间轮转),带 file:line + 证据出 findings
//   2) 裁定:VOTERS 个怀疑型投票者**默认 NOT_REAL** 多轮投票;累计 REAL 占比 ≥ACCEPT 接受、
//      ≤REJECT 拒绝、中间(近 50/50)进下一轮追加投票,到 MAX_VOTE_ROUNDS 仍未决则拒绝
//   3) 裁决:接受项按 严重度+占比 取 top-N,交一个成员(HIVE_ARBITER_MODEL)从严产出中文 review
//
// 自包含:只用 Node 内置 fetch + gh CLI,无 npm 依赖。默认走 SenseNova OpenAI 兼容端点
// (deepseek-v4-flash + sensenova-6.7-flash-lite 混合),key 从 secrets 注入(支持多 key 轮询);
// 端点/模型/规模均可覆盖。

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";

const BASE_URL = (process.env.HIVE_BASE_URL || "https://token.sensenova.cn/v1").replace(/\/+$/, "");
// 多 key 轮询:HIVE_API_KEYS(逗号分隔)优先,否则收集 HIVE_API_KEY / HIVE_API_KEY_2/3...
// 去重 + 去空,每次请求 round-robin 摊负载,重试时自动切下一把(跳过被限流/失效的 key)。
const KEYS = [
  ...(process.env.HIVE_API_KEYS || "").split(","),
  process.env.HIVE_API_KEY,
  process.env.HIVE_API_KEY_2,
  process.env.HIVE_API_KEY_3,
]
  .map((s) => (s || "").trim())
  .filter(Boolean)
  .filter((k, i, a) => a.indexOf(k) === i);
let keyCursor = 0;
// 累计 token 消耗(按模型),从每次响应的 usage 字段取
const usageByModel = {};
// 多模型混合 hive:成员在这几个模型间轮转,同台 debate(HIVE_MODEL 单数仍兼容)。
const MODELS = (process.env.HIVE_MODELS || process.env.HIVE_MODEL || "deepseek-v4-flash,sensenova-6.7-flash-lite")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);
const ARBITER_MODEL = process.env.HIVE_ARBITER_MODEL || MODELS[0];
const SWARM = Math.max(1, parseInt(process.env.HIVE_SWARM_SIZE || "10", 10));
const CONCURRENCY = Math.max(1, parseInt(process.env.HIVE_CONCURRENCY || "12", 10));
const PR = process.env.PR_NUMBER;
const MAX_DIFF = parseInt(process.env.HIVE_MAX_DIFF || "200000", 10);

// 严格收敛裁定参数(少而准):默认 NOT_REAL 的投票者多轮裁定,累计占比分流
const VOTERS = Math.max(1, parseInt(process.env.HIVE_VOTERS || "12", 10)); // 每轮投票者数量
const ACCEPT_RATIO = parseFloat(process.env.HIVE_ACCEPT_RATIO || "0.7"); // 累计 REAL 占比 ≥ 此值 → 接受
const REJECT_RATIO = parseFloat(process.env.HIVE_REJECT_RATIO || "0.35"); // ≤ 此值 → 拒绝;中间(近 50/50)→ 进下一轮
const MAX_VOTE_ROUNDS = Math.max(1, parseInt(process.env.HIVE_MAX_VOTE_ROUNDS || "3", 10));
const TOP_N = Math.max(1, parseInt(process.env.HIVE_TOP_N || "8", 10)); // 交总裁判的最多 finding 数

if (!PR) {
  console.error("PR_NUMBER 未设置");
  process.exit(1);
}
if (KEYS.length === 0) {
  console.error("没有可用 key(在 repo secrets 添加 HIVE_API_KEY,多 key 再加 HIVE_API_KEY_2 或用 HIVE_API_KEYS 逗号分隔)");
  process.exit(1);
}
console.error(
  `hive-think: PR #${PR} · ${SWARM} finders / ${VOTERS} voters×≤${MAX_VOTE_ROUNDS}轮 · ` +
    `接受≥${ACCEPT_RATIO}/拒绝≤${REJECT_RATIO} · top${TOP_N} · 模型 [${MODELS.join(", ")}] · 裁决 ${ARBITER_MODEL} · ${KEYS.length} key`,
);

// ── 项目准则(精简版,塞进每个成员的上下文,让它知道该查什么)──────────────
const GUIDELINES = `
## Bong 项目审核准则(末法残土修仙沙盒,三层架构 server/Rust + agent/TS + client/Java)

### 世界观锚定(docs/worldview.md 是正典,代码不得矛盾)
- 六境界:醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚。禁用"筑基/金丹/元婴"等传统称谓。
- 货币:骨币(通货)、灵石(燃料,非货币)。不得出现"灵石=钱"的逻辑。
- 灵气守恒律:全服灵气总量恒定。真元流动必须走 qi_physics::QiTransfer{from,to,amount}。红旗:
  qi_current += X 无对应 zone 减 / zone.spirit_qi -= Y 无对应玩家增 / 衰变让真元凭空消失 / 招式只扣攻方不写环境。
- 物理常数唯一源:衰减/逸散/半衰常数必须来自 qi_physics,禁止各 plan 硬编 *_DECAY*/*_DRAIN*/0.0X_f64。
- 命名"末法去上古",不要仙气飘飘。

### 架构硬约束
- 跨层通信只走 Redis IPC + CustomPayload。
- IPC schema:TypeBox(TS)是 source of truth → JSON Schema → Rust serde,改 schema 必须双端同步。
- 新增 SkillRegistry::register 必须同步在 cultivation::meridian::severed::SkillMeridianDependencies::declare 注册依赖经脉。
- Bevy ECS:component 是数据、system 是逻辑,别在 component 写逻辑方法。

### 代码与测试
- 简单易懂 > 花里胡哨;过度抽象/无用注释/超出 plan 的功能蔓延都是减分项。
- 测契约不测实现;测试要饱和:happy path + 所有边界 + 所有错误分支 + 所有状态转换。
`.trim();

// ── 10 个成员维度(swarm 大小不同时循环复用)──────────────────────────────
const PERSONAS = [
  { key: "正确性", focus: "逻辑错误、算法正确性、返回值与边界 off-by-one" },
  { key: "错误分支", focus: "错误处理、unwrap/expect/panic、空值、未覆盖的失败路径" },
  { key: "并发与状态机", focus: "数据竞争、锁、async 顺序、enum 状态转换遗漏、生命周期" },
  { key: "安全", focus: "命令注入、不安全反序列化、未校验外部输入、权限绕过" },
  { key: "性能", focus: "Rust 不必要的 clone/alloc、Redis 调用频率、worldgen O(n^2) 热路径" },
  { key: "架构一致性", focus: "三层架构边界、Bevy ECS 数据/逻辑分离、LAYER_REGISTRY、SkillRegistry::register 同步 declare 依赖经脉" },
  { key: "世界观对齐", focus: "六境界命名、骨币/灵石、末法去上古风格" },
  { key: "灵气守恒", focus: "真元增减是否走 qi_physics::QiTransfer、灵气总量守恒、凭空增减红旗、物理常数来源" },
  { key: "IPC schema 对齐", focus: "TypeBox(TS)↔ JSON Schema ↔ Rust serde ↔ client payload 双端/三端同步" },
  { key: "测试饱和与简洁度", focus: "happy/边界/错误分支/状态转换覆盖、过度抽象、功能蔓延、冗余注释" },
];
// 每个成员 = 一个维度 + 一个模型(都 round-robin):维度全覆盖,模型在成员间均摊,同台 debate
const swarm = Array.from({ length: SWARM }, (_, i) => ({
  ...PERSONAS[i % PERSONAS.length],
  model: MODELS[i % MODELS.length],
}));

// ── 工具 ───────────────────────────────────────────────────────────────────
function gh(args) {
  return execSync(`gh ${args}`, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

// 有限并发的 map,避免一次性把端点打到 429
async function mapLimit(items, limit, fn) {
  const out = new Array(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      out[i] = await fn(items[i], i);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return out;
}

// 单次 chat,带超时 + 429/网络错误重试退避;每次尝试 round-robin 换一把 key(失败自动切下一把)
async function chat(content, { label = "", model = MODELS[0], retries = Math.max(4, KEYS.length * 2) } = {}) {
  for (let attempt = 1; attempt <= retries; attempt++) {
    const key = KEYS[keyCursor++ % KEYS.length];
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 300_000);
    try {
      const res = await fetch(`${BASE_URL}/chat/completions`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${key}` },
        body: JSON.stringify({ model, messages: [{ role: "user", content }] }),
        signal: ctrl.signal,
      });
      clearTimeout(timer);
      if (!res.ok) {
        const body = (await res.text()).slice(0, 300);
        throw new Error(`HTTP ${res.status} ${body}`);
      }
      const data = await res.json();
      const u = data?.usage;
      if (u) {
        const m = usageByModel[model] || (usageByModel[model] = { prompt: 0, completion: 0, total: 0, calls: 0 });
        m.prompt += u.prompt_tokens || 0;
        m.completion += u.completion_tokens || 0;
        m.total += u.total_tokens || (u.prompt_tokens || 0) + (u.completion_tokens || 0);
        m.calls += 1;
      }
      const text = data?.choices?.[0]?.message?.content;
      if (!text || !text.trim()) throw new Error("空响应");
      return text;
    } catch (e) {
      clearTimeout(timer);
      // key 只露尾 4 位,不泄全量
      console.error(`[${label}] 第 ${attempt}/${retries} 次失败(key …${key.slice(-4)}): ${e.message}`);
      if (attempt === retries) return null;
      await sleep(2500 * attempt);
    }
  }
  return null;
}

// 从模型输出里抠出第一段完整 JSON(容忍 ```json 围栏与前后废话),balanced-bracket 扫描
function extractJSON(text) {
  if (!text) return null;
  let t = text.trim();
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

// ── 收集 PR 上下文 ──────────────────────────────────────────────────────────
let meta;
try {
  meta = JSON.parse(gh(`pr view ${PR} --json title,body,headRefName,files`));
} catch (e) {
  console.error("取 PR 元信息失败:", e.message);
  process.exit(1);
}

let diff = "";
try {
  diff = gh(`pr diff ${PR}`);
} catch (e) {
  console.error("取 diff 失败:", e.message);
  process.exit(1);
}
let diffTruncated = false;
if (diff.length > MAX_DIFF) {
  diff = diff.slice(0, MAX_DIFF);
  diffTruncated = true;
}

const fileList = (meta.files || [])
  .map((f) => `- ${f.path} (+${f.additions}/-${f.deletions})`)
  .join("\n");

// plan-aware:从标题/分支名探测 plan,读对应文件喂给成员
function loadPlan() {
  const m = `${meta.title} ${meta.headRefName}`.match(/plan-[a-z0-9-]+-v\d+/i);
  if (!m) return null;
  const name = m[0];
  for (const dir of ["docs", "docs/finished_plans", "docs/plans-skeleton"]) {
    const p = `${dir}/${name}.md`;
    if (existsSync(p)) return { name, path: p, text: readFileSync(p, "utf8").slice(0, 40000) };
  }
  return { name, path: null, text: null };
}
const plan = loadPlan();
const planBlock = plan?.text
  ? `\n## 关联 Plan(${plan.path})\n${plan.text}\n`
  : plan
    ? `\n## 关联 Plan: ${plan.name}(未在仓库找到对应文件,按非 plan PR 处理)\n`
    : "";

const prContext = `
## PR #${PR}: ${meta.title}
${(meta.body || "").slice(0, 4000)}

## 变更文件
${fileList || "(无)"}
${planBlock}
## 完整 diff${diffTruncated ? `(已截断至 ${MAX_DIFF} 字符)` : ""}
\`\`\`diff
${diff}
\`\`\`
`.trim();

// 模型可能直接给数组,也可能包成 {"findings":[...]};都归一成数组
function toFindings(raw) {
  const j = extractJSON(raw);
  if (Array.isArray(j)) return j;
  if (j && Array.isArray(j.findings)) return j.findings;
  return [];
}

// ── 第一轮:发现 ────────────────────────────────────────────────────────────
console.error(`▶ 第一轮 发现:${SWARM} 个成员并行(并发 ${CONCURRENCY})`);
const findPrompt = (p) =>
  `你是 Bong 项目 PR 审核 hive 的一员,本轮专攻【${p.key}】维度(关注:${p.focus})。\n` +
  `${GUIDELINES}\n\n${prContext}\n\n` +
  `只从你负责的【${p.key}】维度找**真问题**,每条必须能在 diff 中定位到 file:line 并引用具体代码作证据。\n` +
  `不要风格洁癖凑数,没有问题就返回空数组。行号取自 diff,不要编造。\n` +
  `只输出纯 JSON 数组(不要任何解释文字):\n` +
  `[{"severity":"blocker|major|minor","file":"路径","line":"行号或范围","title":"一句话问题","evidence":"diff 中的具体代码片段","why":"为什么是问题"}]`;

const round1 = await mapLimit(swarm, CONCURRENCY, (p) =>
  chat(findPrompt(p), { label: `find:${p.key}@${p.model}`, model: p.model }).then((r) => ({
    persona: p.key,
    model: p.model,
    findings: toFindings(r),
  })),
);

const pool = [];
round1.forEach((r) => {
  r.findings.forEach((f) => {
    if (f && (f.title || f.file)) {
      pool.push({ ...f, by: r.persona, model: r.model, id: pool.length + 1 });
    }
  });
});
console.error(`  发现 ${pool.length} 条 findings`);

// ── 多轮裁定:默认 NOT_REAL 的怀疑型投票者,累计 REAL 占比分流 ────────────────
// 接受(≥ACCEPT)/ 拒绝(≤REJECT)/ 中间近 50/50 有争议 → 追加下一轮投票,到顶仍未决则拒绝。
const foundCount = pool.length; // finder 原始数(missed 后续会进 pool)
const tally = {};
pool.forEach((f) => (tally[f.id] = { real: 0, not: 0 }));
const decision = {}; // id -> 'accept' | 'reject'
const roundsUsed = {}; // id -> 最后收到票的轮次
let missed = [];

// 投票者:VOTERS 个,模型轮转(与 finder 各包维度不同,投票者只做裁断)
const voters = Array.from({ length: VOTERS }, (_, i) => ({ model: MODELS[i % MODELS.length] }));

const votePrompt = (activeFindings, askMissed) => {
  const view = activeFindings.map(({ id, severity, file, line, title, why }) => ({ id, severity, file, line, title, why }));
  return (
    `你是 Bong PR 审核 hive 的**怀疑型投票者**。\n${GUIDELINES}\n\n${prContext}\n\n` +
    `逐条裁断下面的 findings(带 id)。**默认判 NOT_REAL**——只有当你能在 diff 中明确确认全部四点:\n` +
    `① 问题真实存在 ② 该代码路径可达 ③ 周围代码确实没有处理 ④ 行号对得上,才投 REAL。\n` +
    `证据不足、无法定位、推测性、风格洁癖、"可能/也许/建议"一律 NOT_REAL。宁可漏过,也不要放过假阳性。\n\n` +
    `findings:\n${JSON.stringify(view, null, 2)}\n\n` +
    (askMissed ? `如确有**所有人都漏掉、且证据确凿**的真问题,补进 missed(没有就空数组,严禁硬凑)。\n` : ``) +
    `只输出纯 JSON(不要解释文字):\n` +
    `{"votes":[{"id":1,"verdict":"REAL|NOT_REAL"}]` +
    (askMissed ? `,"missed":[{"severity":"blocker|major|minor","file":"路径","line":"行号","title":"...","why":"..."}]` : ``) +
    `}`
  );
};

let active = pool.map((f) => f.id);
for (let round = 1; round <= MAX_VOTE_ROUNDS && active.length > 0; round++) {
  const activeFindings = pool.filter((f) => active.includes(f.id));
  console.error(`▶ 裁定第 ${round}/${MAX_VOTE_ROUNDS} 轮:${VOTERS} 投票者 × ${activeFindings.length} 条未决`);
  const askMissed = round === 1; // 仅首轮收 missed,避免无限膨胀
  const results = await mapLimit(voters, CONCURRENCY, (v, i) =>
    chat(votePrompt(activeFindings, askMissed), { label: `vote.r${round}#${i}@${v.model}`, model: v.model }).then(
      (r) => extractJSON(r) || { votes: [], missed: [] },
    ),
  );

  for (const r of results) {
    for (const v of r?.votes || []) {
      const t = tally[v?.id];
      if (!t) continue;
      if (String(v.verdict || "").toUpperCase().includes("NOT")) t.not++;
      else t.real++;
    }
    if (askMissed) {
      for (const m of r?.missed || []) {
        if (m && (m.title || m.file)) {
          const f = { ...m, by: "补漏", model: "—", id: pool.length + 1 };
          pool.push(f);
          tally[f.id] = { real: 0, not: 0 };
          active.push(f.id); // 下一轮起参与投票
        }
      }
    }
  }
  missed = pool.filter((f) => f.by === "补漏");

  // 累计占比分流
  const stillActive = [];
  for (const id of active) {
    const t = tally[id];
    const total = t.real + t.not;
    if (total === 0) {
      stillActive.push(id); // 本轮没票(如刚补漏的),下一轮再投
      continue;
    }
    roundsUsed[id] = round;
    const ratio = t.real / total;
    if (ratio >= ACCEPT_RATIO) decision[id] = "accept";
    else if (ratio <= REJECT_RATIO) decision[id] = "reject";
    else stillActive.push(id); // 近 50/50,争议 → 下一轮追加投票
  }
  active = stillActive;
}
for (const id of active) decision[id] = "reject"; // 到顶仍未决 → 拒绝(拿不准就不报)

const ratioOf = (id) => {
  const t = tally[id] || { real: 0, not: 0 };
  const total = t.real + t.not;
  return total ? t.real / total : 0;
};
const pct = (id) => Math.round(ratioOf(id) * 100);

const accepted = pool.filter((f) => decision[f.id] === "accept");
const sevRank = { blocker: 0, major: 1, minor: 2 };
const ranked = accepted
  .slice()
  .sort((a, b) => (sevRank[a.severity] ?? 3) - (sevRank[b.severity] ?? 3) || ratioOf(b.id) - ratioOf(a.id));
const survivors = ranked.slice(0, TOP_N); // 交总裁判的 top-N(高置信度)
const overflow = ranked.slice(TOP_N); // 接受但超出 top-N
console.error(
  `  接受 ${accepted.length} / 拒绝 ${pool.length - accepted.length} / 交裁判 top${survivors.length}` +
    (overflow.length ? `(另 ${overflow.length} 条接受但超 top-N)` : ""),
);

// ── 总裁决(从严,只就 top-N)────────────────────────────────────────────────
console.error("▶ 总裁决:总裁判综合产出");
const fmt = (f) =>
  `- [${f.severity || "?"}] ${f.file || "?"}:${f.line || "?"} — ${f.title || ""}` +
  `${f.why ? `(${f.why})` : ""} 〔认可 ${pct(f.id)}% · ${tally[f.id].real}✓/${tally[f.id].not}✗〕`;

const arbiterPrompt =
  `你是本次 hive-think 审核的**总裁判**(最终裁决,从严,宁缺勿滥)。\n` +
  `${GUIDELINES}\n\n${prContext}\n\n` +
  `经过 ${VOTERS} 个怀疑型投票者最多 ${MAX_VOTE_ROUNDS} 轮裁定(默认 NOT_REAL,累计 REAL 占比 ≥${ACCEPT_RATIO} 才接受),` +
  `**高置信度存活的 top-${TOP_N} findings**:\n${survivors.map(fmt).join("\n") || "(无)"}\n\n` +
  `据此产出**最终中文 PR review(markdown)**。硬要求:\n` +
  `- **只就上面这些高置信度问题写**,严禁自行新增未列出的问题,严禁凑数。\n` +
  `- 每条带 file:line 与证据;其中你判断明显不成立的可直接否决(从严)。\n` +
  `- 结构:**整体评级**(✅ 可合入 / ⚠️ 有阻塞)→ 🚫 阻塞项 → 🐛 主要问题 → 📐 次要/建议${plan?.text ? " → 📋 Plan 对齐度" : ""}。\n` +
  `- 没有高置信度问题就直接写「未发现高置信度阻塞问题」+ 一句说明,**不要硬找问题**。\n` +
  `- 不要包含本提示词、不要输出 JSON,直接给 markdown 正文。`;

let finalReview = await chat(arbiterPrompt, { label: `arbiter@${ARBITER_MODEL}`, model: ARBITER_MODEL });
if (!finalReview) {
  finalReview =
    `_总裁决阶段无输出,以下为高置信度 top-${TOP_N} 直出:_\n\n` +
    (survivors.length ? survivors.map(fmt).join("\n") : "未发现高置信度问题。");
}

// ── 组装并发布 ──────────────────────────────────────────────────────────────
const statusOf = (f) => (survivors.includes(f) ? "🎯 top" : decision[f.id] === "accept" ? "✅ 接受" : "❌ 拒绝");
const voteRows = pool
  .slice()
  .sort((a, b) => ratioOf(b.id) - ratioOf(a.id))
  .map((f) => {
    const t = tally[f.id] || { real: 0, not: 0 };
    return `| ${f.id} | ${f.by || "?"} | ${f.model || "?"} | ${(f.title || "").replace(/\|/g, "/")} | ${pct(f.id)}% (${t.real}✓/${t.not}✗) | ${roundsUsed[f.id] || "-"} | ${statusOf(f)} |`;
  })
  .join("\n");

const debateTable = pool.length
  ? `\n\n<details><summary>🗳️ 裁定明细(${VOTERS} 投票者 ×≤${MAX_VOTE_ROUNDS} 轮 · 接受≥${Math.round(ACCEPT_RATIO * 100)}% / 拒绝≤${Math.round(REJECT_RATIO * 100)}%)</summary>\n\n` +
    `| id | 提出 | 模型 | 问题 | 认可率 | 轮 | 结果 |\n|---|---|---|---|---|---|---|\n${voteRows}\n</details>`
  : "";

// 成员里每个模型各占几个,供 header 展示(顺带把两个模型的产出对比放进投票表)
const modelMix = MODELS.map((m) => `\`${m}\`×${swarm.filter((s) => s.model === m).length}`).join(" + ");
const header =
  `## 🐝 Hive-Think Review · PR #${PR}\n\n` +
  `> 模型 **\`hive-think\`** = ${SWARM} finders + ${VOTERS} 怀疑投票者(${modelMix}),裁决 \`${ARBITER_MODEL}\`\n` +
  `> 流程:发现 → 默认 NOT_REAL 多轮投票(接受≥${Math.round(ACCEPT_RATIO * 100)}% / 拒绝≤${Math.round(REJECT_RATIO * 100)}% / 争议进下一轮)→ top-${TOP_N} 总裁决\n` +
  (plan ? `> Plan: \`${plan.name}\`${plan.path ? "" : "(未找到文件)"}\n` : "") +
  (diffTruncated ? `> ⚠️ diff 过大已截断至 ${MAX_DIFF} 字符\n` : "") +
  `> 统计:发现 ${foundCount}${missed.length ? `(+${missed.length} 补漏)` : ""} → 接受 ${accepted.length} → 交裁判 top-${survivors.length}\n\n` +
  `> [!WARNING]\n` +
  `> 本审核由多模型自动 debate 生成,**不可 100% 信赖**——快模型可能幻觉/误报/漏报。\n` +
  `> 请保持批判性思维,务必自行核对 file:line 与上下文,再决定是否采纳。\n\n`;

// token 消耗页脚(末尾)——从各响应 usage 累加
const grand = Object.values(usageByModel).reduce(
  (a, m) => ({ prompt: a.prompt + m.prompt, completion: a.completion + m.completion, total: a.total + m.total, calls: a.calls + m.calls }),
  { prompt: 0, completion: 0, total: 0, calls: 0 },
);
const num = (n) => n.toLocaleString("en-US");
const perModel = Object.entries(usageByModel)
  .sort((a, b) => b[1].total - a[1].total)
  .map(([m, u]) => `\`${m}\` ${num(u.total)}(${u.calls} 次)`)
  .join(" · ");
const tokenFooter =
  `\n\n---\n` +
  `📊 **本次 token 消耗**:总 **${num(grand.total)}**` +
  `(prompt ${num(grand.prompt)} + completion ${num(grand.completion)})· ${grand.calls} 次模型调用` +
  (perModel ? `\n> ${perModel}` : "") +
  `\n`;

const body = header + finalReview + debateTable + tokenFooter;

writeFileSync("/tmp/hive-review.md", body);
if (process.env.HIVE_DRY_RUN) {
  console.error("— HIVE_DRY_RUN:不发评论,正文如下 —");
  console.log(body);
} else {
  try {
    gh(`pr comment ${PR} --body-file /tmp/hive-review.md`);
    console.error("✅ 已发布 review 评论");
  } catch (e) {
    console.error("发布评论失败:", e.message);
    console.error(body);
    process.exit(1);
  }
}
