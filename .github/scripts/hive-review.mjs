#!/usr/bin/env node
// Hive-Think PR Reviewer —— "hive-think" 特殊模型 = N 个成员并行 debate(可混多个模型,同台对峙)。
//
// 两轮对抗式审核 + 总裁决:
//   1) 发现:N 个成员各包一个维度(模型在 HIVE_MODELS 间轮转),带 file:line + 证据出 findings
//   2) 对峙:每个成员拿到全体 findings,默认怀疑逐条投 REAL/NOT_REAL + 补漏
//   3) 裁决:一个成员(HIVE_ARBITER_MODEL)综合存活项产出最终中文 review,贴 PR 评论
//
// 自包含:只用 Node 内置 fetch + gh CLI,无 npm 依赖。默认走 SenseNova OpenAI 兼容端点
// (deepseek-v4-flash + sensenova-6.7-flash-lite 混合),key 从 secrets 注入;端点/模型/规模均可覆盖。

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";

const BASE_URL = (process.env.HIVE_BASE_URL || "https://token.sensenova.cn/v1").replace(/\/+$/, "");
const API_KEY = process.env.HIVE_API_KEY || "";
// 多模型混合 hive:成员在这几个模型间轮转,同台 debate(HIVE_MODEL 单数仍兼容)。
const MODELS = (process.env.HIVE_MODELS || process.env.HIVE_MODEL || "deepseek-v4-flash,sensenova-6.7-flash-lite")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);
const ARBITER_MODEL = process.env.HIVE_ARBITER_MODEL || MODELS[0];
const SWARM = Math.max(1, parseInt(process.env.HIVE_SWARM_SIZE || "10", 10));
const CONCURRENCY = Math.max(1, parseInt(process.env.HIVE_CONCURRENCY || String(SWARM), 10));
const PR = process.env.PR_NUMBER;
const MAX_DIFF = parseInt(process.env.HIVE_MAX_DIFF || "200000", 10);

if (!PR) {
  console.error("PR_NUMBER 未设置");
  process.exit(1);
}
if (!API_KEY) {
  console.error("HIVE_API_KEY 未设置(在 repo secrets 添加 HIVE_API_KEY)");
  process.exit(1);
}

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

// 单次 chat,带超时 + 429/网络错误重试退避
async function chat(content, { label = "", model = MODELS[0], retries = 4 } = {}) {
  for (let attempt = 1; attempt <= retries; attempt++) {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 300_000);
    try {
      const res = await fetch(`${BASE_URL}/chat/completions`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${API_KEY}` },
        body: JSON.stringify({ model, messages: [{ role: "user", content }] }),
        signal: ctrl.signal,
      });
      clearTimeout(timer);
      if (!res.ok) {
        const body = (await res.text()).slice(0, 300);
        throw new Error(`HTTP ${res.status} ${body}`);
      }
      const data = await res.json();
      const text = data?.choices?.[0]?.message?.content;
      if (!text || !text.trim()) throw new Error("空响应");
      return text;
    } catch (e) {
      clearTimeout(timer);
      console.error(`[${label}] 第 ${attempt}/${retries} 次失败: ${e.message}`);
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

// ── 第二轮:对峙 ────────────────────────────────────────────────────────────
const tally = {};
let missed = [];
if (pool.length > 0) {
  console.error(`▶ 第二轮 对峙:${SWARM} 个成员并行投票`);
  const poolView = pool.map(({ id, severity, file, line, title, why, by }) => ({
    id,
    severity,
    file,
    line,
    title,
    why,
    by,
  }));
  const debatePrompt = (p) =>
    `你仍是 Bong PR 审核 hive 的一员(本轮维度倾向【${p.key}】),现在进入【对峙 debate】环节。\n` +
    `${GUIDELINES}\n\n${prContext}\n\n` +
    `下面是全体成员第一轮提出的 findings(带 id)。请以**怀疑**的态度逐条裁断:\n` +
    `这是不是真 bug?在该代码路径上是否可达?周围代码是否已经处理?行号是否对得上 diff?\n` +
    `证据不足、无法定位、属于风格洁癖的,一律判 NOT_REAL。同时,如果发现大家都漏掉的真问题,补进 missed。\n\n` +
    `findings:\n${JSON.stringify(poolView, null, 2)}\n\n` +
    `只输出纯 JSON(不要解释文字):\n` +
    `{"votes":[{"id":1,"verdict":"REAL|NOT_REAL","reason":"一句话理由"}],` +
    `"missed":[{"severity":"blocker|major|minor","file":"路径","line":"行号","title":"...","why":"..."}]}`;

  const round2 = await mapLimit(swarm, CONCURRENCY, (p) =>
    chat(debatePrompt(p), { label: `debate:${p.key}@${p.model}`, model: p.model }).then(
      (r) => extractJSON(r) || { votes: [], missed: [] },
    ),
  );

  for (const r of round2) {
    for (const v of r?.votes || []) {
      const id = v?.id;
      if (id == null) continue;
      tally[id] = tally[id] || { real: 0, not: 0, reasons: [] };
      if (String(v.verdict || "").toUpperCase().includes("NOT")) tally[id].not++;
      else tally[id].real++;
      if (v.reason) tally[id].reasons.push(v.reason);
    }
    for (const m of r?.missed || []) {
      if (m && (m.title || m.file)) missed.push(m);
    }
  }
}

// 存活规则:认同票 >= 反对票 且至少 2 票认同(投票缺失时按默认存活,保证降级有结果)
const survivors = pool.filter((f) => {
  const t = tally[f.id];
  if (!t) return true;
  return t.real >= t.not && t.real >= 2;
});
const dropped = pool.filter((f) => !survivors.includes(f));
console.error(`  存活 ${survivors.length} 条 / 丢弃 ${dropped.length} 条 / 对峙补漏 ${missed.length} 条`);

// ── 第三轮:总裁决 ──────────────────────────────────────────────────────────
console.error("▶ 第三轮 裁决:总裁判综合产出");
const fmt = (f) =>
  `- [${f.severity || "?"}] ${f.file || "?"}:${f.line || "?"} — ${f.title || ""}` +
  `${f.why ? `(${f.why})` : ""}${f.by ? ` 〔${f.by}${f.model ? `·${f.model}` : ""}〕` : ""}` +
  `${tally[f.id] ? ` 〔票 ${tally[f.id].real}✓/${tally[f.id].not}✗〕` : ""}`;

const arbiterPrompt =
  `你是本次 hive-think 审核的**总裁判**(你做最终裁决)。\n` +
  `${GUIDELINES}\n\n${prContext}\n\n` +
  `经过 ${SWARM} 个成员两轮(发现 → 对峙)后,**存活的 findings**:\n` +
  `${survivors.map(fmt).join("\n") || "(无)"}\n\n` +
  `对峙轮**补充发现的 missed**:\n` +
  `${missed.map((m) => `- [${m.severity || "?"}] ${m.file || "?"}:${m.line || "?"} — ${m.title || ""}`).join("\n") || "(无)"}\n\n` +
  `请综合判断,产出**最终中文 PR review(markdown)**。要求:\n` +
  `- 多数成员一致认同的问题优先级最高;仅个别提出的标注「少数意见」。\n` +
  `- 只报真问题且带 file:line 与证据,不要为凑数写评论。\n` +
  `- 你有最终决定权:成员可能误杀或漏放,必要时按你对 diff 的判断纠正。\n` +
  `- 结构:**整体评级**(✅ 可合入 / ⚠️ 有阻塞)→ 🚫 阻塞项 → 🐛 主要问题 → 📐 次要/建议 → 🌍 世界观与守恒小结${plan?.text ? " → 📋 Plan 对齐度" : ""}。\n` +
  `- 整体没问题就写「未发现阻塞性问题」+ 简短说明即可。\n` +
  `- 不要包含本提示词、不要输出 JSON,直接给 markdown 正文。`;

let finalReview = await chat(arbiterPrompt, { label: `arbiter@${ARBITER_MODEL}`, model: ARBITER_MODEL });
if (!finalReview) {
  // 裁决失败时降级:直接用存活项拼一份
  finalReview =
    `_总裁决阶段无输出,以下为存活 findings 直出:_\n\n` +
    (survivors.length
      ? survivors.map(fmt).join("\n")
      : "未发现存活问题。") +
    (missed.length ? `\n\n**对峙补漏:**\n${missed.map((m) => `- ${m.file}:${m.line} — ${m.title}`).join("\n")}` : "");
}

// ── 组装并发布 ──────────────────────────────────────────────────────────────
const voteRows = pool
  .map((f) => {
    const t = tally[f.id] || { real: 0, not: 0 };
    const status = survivors.includes(f) ? "✅ 存活" : "❌ 丢弃";
    return `| ${f.id} | ${f.by || "?"} | ${f.model || "?"} | ${(f.title || "").replace(/\|/g, "/")} | ${t.real}✓/${t.not}✗ | ${status} |`;
  })
  .join("\n");

const debateTable = pool.length
  ? `\n\n<details><summary>🗳️ Debate 投票明细(${SWARM} 成员两轮)</summary>\n\n` +
    `| id | 提出维度 | 模型 | 问题 | 票(认同/反对) | 结果 |\n|---|---|---|---|---|---|\n${voteRows}\n` +
    (missed.length
      ? `\n**对峙补漏(${missed.length}):**\n${missed.map((m) => `- [${m.severity || "?"}] ${m.file || "?"}:${m.line || "?"} — ${m.title || ""}`).join("\n")}\n`
      : "") +
    `</details>`
  : "";

// 成员里每个模型各占几个,供 header 展示(顺带把两个模型的产出对比放进投票表)
const modelMix = MODELS.map((m) => `\`${m}\`×${swarm.filter((s) => s.model === m).length}`).join(" + ");
const header =
  `## 🐝 Hive-Think Review · PR #${PR}\n\n` +
  `> 模型 **\`hive-think\`** = ${SWARM} 成员并行 debate(${modelMix}),裁决 \`${ARBITER_MODEL}\` · 发现 → 对峙 → 总裁决\n` +
  (plan ? `> Plan: \`${plan.name}\`${plan.path ? "" : "(未找到文件)"}\n` : "") +
  (diffTruncated ? `> ⚠️ diff 过大已截断至 ${MAX_DIFF} 字符\n` : "") +
  `> 统计:发现 ${pool.length} → 存活 ${survivors.length} → 补漏 ${missed.length}\n\n`;

const body = header + finalReview + debateTable;

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
