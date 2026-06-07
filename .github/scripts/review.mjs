#!/usr/bin/env node
// Review v2 —— PR 评论 `/review` 触发(startsWith,见 review.yml)。SDK agent 编排 + 模型别名映射。
//
// 架构:opus(经 ANTHROPIC_DEFAULT_OPUS_MODEL 映射到 deepseek-v4-pro)当 orchestrator + 裁判,
// 用 Task 工具自主 spawn 管理 sonnet(映射到 deepseek-v4-flash) finder/voter 子代理 swarm。
// 全走自家代理 proxy.kizun4.uk(ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN,review.yml 注入)。
// 不再用 codex(responses 端点跑 gpt-5.5 持续不稳)——全 claude harness + 映射。
//
// 混合编排(保留自主性 + 有约束):脚本按 diff 分档算 finder **预算** + 提供几个**玩法模板**
// (多维对峙 / 怀疑裁决 / 低自信 fan-out / 代码断链 wiring),注入 orchestrator prompt;
// opus 在预算 + 玩法约束内**自主调度** flash swarm,自己当裁判汇总出最终中文 review。
//
// 自包含:Node 内置模块 + `gh` + `@anthropic-ai/claude-agent-sdk`。
// 纯逻辑(pickTier / extractJSON / tierBudget / sumModelUsage …)导出供 review.test.mjs 用 `node --test` 锁行为。

import { query } from "@anthropic-ai/claude-agent-sdk";
import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";

// ── 配置(全 env 可覆盖)────────────────────────────────────────────────────────
const PR = process.env.PR_NUMBER;
const DRY_RUN = /^(1|true|yes)$/i.test(String(process.env.REVIEW_DRY_RUN || "").trim());
// opus 要编排 + 等多个子代理 + 汇总,主轮数给足;子代理限轮防小模型工具循环。
const MAX_TURNS = Math.max(8, parseInt(process.env.REVIEW_MAX_TURNS || "40", 10));
const FINDER_MAX_TURNS = Math.max(2, parseInt(process.env.REVIEW_FINDER_MAX_TURNS || "10", 10));
// orchestrator(贵 pro)prompt 里的 diff 只作概览,截断更狠省 token;细节由 finder 子代理读真实文件补全。
const MAX_DIFF = parseInt(process.env.REVIEW_MAX_DIFF || "100000", 10);
// 模型别名:默认用 opus/sonnet,由 review.yml 的 ANTHROPIC_DEFAULT_OPUS_MODEL/SONNET_MODEL 映射到 deepseek pro/flash。
// 想直接钉死模型 id 就设 REVIEW_OPUS_MODEL=deepseek-v4-pro 等。
const OPUS = process.env.REVIEW_OPUS_MODEL || "opus";
const SONNET = process.env.REVIEW_SONNET_MODEL || "sonnet";
const _finderPin = parseInt(process.env.REVIEW_FINDERS || "", 10);
const FINDER_PIN = Number.isFinite(_finderPin) ? Math.max(1, _finderPin) : null; // 非法/空 → null(用档位默认)

// ══════════════════════════════════════════════════════════════════════════════
//  纯逻辑(导出测试)
// ══════════════════════════════════════════════════════════════════════════════

// 动态分档:按总变更行数定 finder **预算**(opus 自主调度,但不超此上限,控成本)。
// SDK 子代理并发上限 10,故 finders ≤10。maxLines 升序,末档 Infinity 兜底。
export const TIERS = [
  { label: "trivial", maxLines: 40, finders: 3 },
  { label: "small", maxLines: 250, finders: 4 },
  { label: "medium", maxLines: 900, finders: 6 },
  { label: "large", maxLines: 2500, finders: 8 },
  { label: "huge", maxLines: Infinity, finders: 10 },
];

// 选档:总变更行数定基础档;文件数 ≥ bumpFiles(改动面广)升一档,封顶末档。
export function pickTier(changedLines, changedFiles, { tiers = TIERS, bumpFiles = 15 } = {}) {
  const lines = Number(changedLines) || 0;
  let idx = tiers.findIndex((t) => lines <= t.maxLines);
  if (idx < 0) idx = tiers.length - 1;
  if ((Number(changedFiles) || 0) >= bumpFiles && idx < tiers.length - 1) idx += 1;
  return tiers[idx];
}

// finder 预算:显式 pin 优先,否则取档位 finders,封顶 10(SDK 并发上限)。
export function tierBudget(tier, { pin = FINDER_PIN, cap = 10 } = {}) {
  const n = pin || tier?.finders || 3;
  return Math.max(1, Math.min(cap, n));
}

// 从模型输出抠第一段完整 JSON(容忍 ```json 围栏 + 前后废话),balanced-bracket 扫描。
export function extractJSON(text) {
  if (!text) return null;
  let t = String(text).trim();
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

// 把 SDK result 的 modelUsage 聚合成 [{model, input, output, total, costUSD}],按 total 降序。
export function sumModelUsage(modelUsage) {
  const out = [];
  for (const [model, u] of Object.entries(modelUsage || {})) {
    const input = (u?.inputTokens || 0) + (u?.cacheReadInputTokens || 0) + (u?.cacheCreationInputTokens || 0);
    const output = u?.outputTokens || 0;
    out.push({ model, input, output, total: input + output, costUSD: u?.costUSD || 0 });
  }
  return out.sort((a, b) => b.total - a.total);
}

// ══════════════════════════════════════════════════════════════════════════════
//  审核准则 + 玩法模板(注入 orchestrator + 子代理)
// ══════════════════════════════════════════════════════════════════════════════
const GUIDELINES = `
## Bong 项目审核准则(末法残土修仙沙盒,三层架构 server/Rust + agent/TS + client/Java)

### 世界观锚定(docs/worldview.md 是正典,代码不得矛盾)
- 六境界:醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚。禁用"筑基/金丹/元婴"等传统称谓。
- 货币:骨币(通货)、灵石(燃料,非货币)。不得出现"灵石=钱"的逻辑。
- 灵气守恒律:全服灵气总量恒定。真元流动必须走 qi_physics::QiTransfer{from,to,amount}。红旗:
  qi_current += X 无对应 zone 减 / zone.spirit_qi -= Y 无对应玩家增 / 衰变让真元凭空消失 / 招式只扣攻方不写环境。
  **注意**:若某模式与已合并的 sibling(如 fauna/rat_phase.rs)逐字一致,属既有正典模式,非本 PR 引入,勿当 blocker。
- 物理常数唯一源:衰减/逸散/半衰常数必须来自 qi_physics,禁止各 plan 硬编 *_DECAY*/*_DRAIN*/0.0X_f64。

### 架构硬约束
- 跨层通信走 Redis IPC + CustomPayload。IPC schema:TypeBox(TS)是 server↔agent 的 source of truth → JSON Schema → Rust serde,改 schema 必须双端同步。
  **注意**:server→client(Rust→Java)的 CustomPayload 是另一条通道,全仓既有几十个 *_emit.rs 都手写 serde JSON,不强制走 TypeBox。
- 新增 SkillRegistry::register 必须同步在 cultivation::meridian::severed::SkillMeridianDependencies::declare 注册依赖经脉。
- Bevy ECS:component 是数据、system 是逻辑,别在 component 写逻辑方法。

### 代码与测试
- 简单易懂 > 花里胡哨;过度抽象/无用注释/超出 plan 的功能蔓延都是减分项。
- 测契约不测实现;测试要饱和:happy path + 所有边界 + 所有错误分支 + 所有状态转换。
`.trim();

// 玩法模板:opus 自主选用组合,在 finder 预算内调度 flash 子代理。
const PLAYBOOKS = `
## 可用玩法(在 finder 预算内自主选用、组合)

### 玩法 A · 多维对峙(debate)——默认主玩法
按维度分工 spawn 多个 finder 子代理(每个盯一个维度,并行),各自带工具打开真实文件核对,出带 file:line + 证据的 findings:
- plan 对齐:对照 PR 关联 plan 该阶段交付物,逐项 ✅/❌/⚠️;plan 列出但缺失的模块/函数/测试/schema
- **代码断链 / wiring(重点!)**:见下「玩法 D」,务必单独派一个 finder 专盯
- 灵气守恒:真元增减是否走 qi_physics::QiTransfer、物理常数来源是否唯一
- 正确性:逻辑、边界 off-by-one、并发、错误分支、IPC schema 对齐
- 测试饱和:happy + 边界 + 错误分支 + 状态转换
- 世界观 / 简洁度:六境界命名、骨币灵石、过度抽象、功能蔓延

### 玩法 B · 怀疑裁决(jury)
finder 汇总后,你(orchestrator)当**怀疑型裁判**从严筛:默认 NOT_REAL,只有能在 diff/代码明确确认
①问题真实 ②路径可达 ③周围代码确实没处理 ④行号对得上,才保留。证据不足/推测/风格洁癖一律丢弃。
(可选:对争议项再 spawn voter 子代理独立复投。)

### 玩法 C · 低自信 fan-out
某维度 finder 说"还差信息/拿不准",对该维度再 spawn 一个专项 finder 深挖到底。

### 玩法 D · 代码断链 / wiring(用户重点要求,务必覆盖)
很多 PR 玩法/逻辑都写了,但**缺真正的 implement / 接线**——定义了却没接入运行路径,是孤岛。
派 finder 用 Grep 搜调用方核实,逐条确认这些断链(每条带 file:line + "定义在 X,但无消费方/未接入"):
- 定义了 struct/fn/component/enum variant 但全仓**无调用方/消费方**
- emit event / CustomPayload 但**没有 system / handler 消费**它
- server 加了算子/字段但 **client 没接**渲染/交互(单方向 stub);或反之
- SkillRegistry::register 了但没在 SkillMeridianDependencies::declare 注册依赖经脉
- schema 加了 variant 但 server/agent/client 某端没用上
- plan 承诺"接入 X"但只加了定义、没写接线代码
- 注册表/registry 加了条目但没被加载/遍历调用
方法:找到新增的定义 → Grep 搜它的使用处 → **搜不到使用 = 断链 finding**。这是最容易被漏、最该抓的一类。
`.trim();

// ── 子代理定义(供 opus 用 Task spawn;model:sonnet → 映射 deepseek-flash)──
function buildAgents() {
  const finderPrompt =
    `你是 Bong PR 审核的 finder 子代理(带 Read/Grep/Glob/Bash 工具)。\n${GUIDELINES}\n\n` +
    `按 orchestrator 指派的维度审查 PR。**打开真实文件 + 用 Grep 搜调用方核对**,不要只凭 diff 推测。\n` +
    `每条 finding 必须带 file:line + 证据片段。没把握/推测性的不报。没问题就明说"该维度核查通过"。\n` +
    `尤其留意【代码断链/wiring】:新增定义 → Grep 搜使用处 → 搜不到=断链。`;
  const voterPrompt =
    `你是 Bong PR 审核的怀疑型投票者(带 Read/Grep 工具)。\n${GUIDELINES}\n\n` +
    `对给定 finding 裁断真假:默认 NOT_REAL,只有打开真实文件确认 ①问题真实 ②路径可达 ③周围没处理 ④行号对得上 才 REAL。\n` +
    `证据不足/推测/风格洁癖一律 NOT_REAL。给出 REAL|NOT_REAL + 一句理由。`;
  return {
    finder: {
      description: "按指定维度审查 PR diff,带工具翻真实代码 + Grep 搜调用方,出带 file:line 的 findings(含代码断链 wiring 检查)",
      prompt: finderPrompt,
      model: SONNET,
      tools: ["Read", "Grep", "Glob", "Bash"],
      maxTurns: FINDER_MAX_TURNS,
    },
    voter: {
      description: "怀疑型投票,裁定单条 finding 真假(默认 NOT_REAL)",
      prompt: voterPrompt,
      model: SONNET,
      tools: ["Read", "Grep"],
      maxTurns: FINDER_MAX_TURNS,
    },
  };
}

// ── orchestrator prompt(opus 主 agent)──
function orchestratorPrompt(prContext, tier, budget, planName) {
  return (
    `你是 Bong 项目本次 PR 审核的**总协调者 + 总裁判**(opus,最强推理)。你有 Task 工具可 spawn finder/voter 子代理,也有 Read/Grep 可自己核对。\n` +
    `${GUIDELINES}\n\n${PLAYBOOKS}\n\n` +
    `## 本次任务\n` +
    `审查下面的 PR(规模档 [${tier.label}])。请按【玩法 A 多维对峙】**用 Task 一次性并行 spawn 恰好 ${budget} 个 finder 子代理**,` +
    `每个盯一个维度(务必含一个专盯【玩法 D 代码断链/wiring】的 finder)。\n` +
    `**成本硬约束(重要)**:① 严格**不超过 ${budget} 个** finder、**不要重复 spawn 同一维度**;` +
    `② 只有某 finder 明确回报"信息不足/无法定位"时,才允许额外【玩法 C fan-out】最多 1 个;` +
    `③ **你自己(orchestrator)是贵模型,不要逐行通读全部代码**——把细节核查交给 finder 子代理(它们用 Read/Grep 读真实文件),` +
    `你只负责:分派维度 → 收齐 findings → 按【玩法 B 怀疑裁决】从严筛 → 汇总裁决。\n\n` +
    `## 输出(最后一条消息)\n` +
    `产出**最终中文 PR review(markdown)**,结构(无内容的小节可省略):\n` +
    `**📋 Plan 对齐度**${planName ? `(${planName})` : ""} —— 交付物逐项 ✅/❌/⚠️\n` +
    `**🔌 代码断链/wiring** —— 定义未接入/emit 无消费/单方向 stub;无则写"✅ 未发现断链"\n` +
    `**🌍 世界观合规** · **🐛 Bug 与正确性** · **📐 代码质量** · **🧪 测试** · **💡 改进建议(非阻塞)**\n` +
    `每条带 file:line + 证据。没有高置信度问题就只写简短总结 + "未发现阻塞问题",不要硬找。\n` +
    `直接给 markdown 正文(不要 JSON、不要复述本提示词)。\n\n` +
    prContext
  );
}

// ── IO ──
function gh(args) {
  return execSync(`gh ${args}`, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}
const num = (n) => Number(n || 0).toLocaleString("en-US");

// ══════════════════════════════════════════════════════════════════════════════
//  主流程
// ══════════════════════════════════════════════════════════════════════════════
async function main() {
  // PR 是唯一插进 gh shell 的外部值——强制数字,杜绝注入。
  if (!PR || !/^\d+$/.test(String(PR))) {
    console.error("PR_NUMBER 未设置或非法(必须纯数字)");
    process.exit(1);
  }
  if (!process.env.ANTHROPIC_AUTH_TOKEN && !process.env.ANTHROPIC_API_KEY) {
    console.error("缺 ANTHROPIC_AUTH_TOKEN/ANTHROPIC_API_KEY(review.yml 应注入,值=PI_CLIPROXY_KEY)");
    process.exit(1);
  }

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

  const changedFiles = (meta.files || []).length;
  const changedLines = (meta.files || []).reduce((s, f) => s + (f.additions || 0) + (f.deletions || 0), 0);
  const tier = pickTier(changedLines, changedFiles);
  const budget = tierBudget(tier);
  const fileList = (meta.files || []).map((f) => `- ${f.path} (+${f.additions}/-${f.deletions})`).join("\n");

  // plan 探测
  const pm = `${meta.title} ${meta.headRefName}`.match(/plan-[a-z0-9-]+-v\d+/i);
  let plan = null;
  if (pm) {
    plan = { name: pm[0], path: null };
    for (const dir of ["docs", "docs/finished_plans", "docs/plans-skeleton"]) {
      const p = `${dir}/${pm[0]}.md`;
      if (existsSync(p)) {
        plan.path = p;
        plan.text = readFileSync(p, "utf8").slice(0, 40000);
        break;
      }
    }
  }
  const planBlock = plan?.text
    ? `\n## 关联 Plan(${plan.path})\n${plan.text}\n`
    : plan
      ? `\n## 关联 Plan: ${plan.name}(未在仓库找到文件,按非 plan PR 处理)\n`
      : "";

  const prContext = `
## 待审 PR #${PR}: ${meta.title}
${(meta.body || "").slice(0, 4000)}

## 变更文件(${changedFiles} 个)
${fileList || "(无)"}
${planBlock}
## 完整 diff${diffTruncated ? `(已截断至 ${MAX_DIFF} 字符)` : ""}
\`\`\`diff
${diff}
\`\`\`
`.trim();

  console.error(
    `Review v2: PR #${PR} · 档[${tier.label}] ${changedLines} 行/${changedFiles} 文件 · finder 预算 ${budget} · ` +
      `orchestrator ${OPUS}(→opus 映射) · swarm ${SONNET}(→sonnet 映射) · maxTurns ${MAX_TURNS}`,
  );

  // ── SDK orchestrator:opus 自主 spawn flash swarm ──
  let reviewText = null;
  let modelUsage = {};
  let totalCost = 0;
  const tasks = []; // 子代理 spawn 记录
  let queryErr = null;
  try {
    for await (const m of query({
      prompt: orchestratorPrompt(prContext, tier, budget, plan?.name),
      options: {
        model: OPUS,
        agents: buildAgents(),
        permissionMode: "bypassPermissions",
        maxTurns: MAX_TURNS,
        allowedTools: ["Task", "Read", "Grep", "Glob", "Bash"],
      },
    })) {
      if (m?.type === "system" && m?.subtype === "task_started") {
        tasks.push(m.subagent_type || m.description || "subagent");
        console.error(`  ▶ spawn 子代理: ${m.subagent_type || "?"} — ${(m.description || "").slice(0, 50)}`);
      }
      if (m?.type === "result") {
        if (typeof m.result === "string" && m.result.trim()) reviewText = m.result;
        if (m.modelUsage) modelUsage = m.modelUsage;
        if (typeof m.total_cost_usd === "number") totalCost = m.total_cost_usd;
      }
    }
  } catch (e) {
    queryErr = e;
    console.error("query 异常:", e?.message || e);
  }

  // ── 组装 report ──
  const usageRows = sumModelUsage(modelUsage);
  const grandTotal = usageRows.reduce((s, r) => s + r.total, 0);
  const perModel = usageRows.map((r) => `\`${r.model}\` ${num(r.total)}`).join(" · ");
  const swarmStat = tasks.length ? `opus 实际 spawn ${tasks.length} 个子代理` : "opus 未 spawn 子代理(自查或异常)";

  let body;
  const header =
    `## 🔭 Review · PR #${PR}\n\n` +
    `> 引擎:SDK agent 编排——**opus orchestrator 自主调度 sonnet finder swarm**,全走 proxy(opus/sonnet 实际映射的模型见末尾 token 表)。\n` +
    `> 档 [${tier.label}](${changedLines} 行/${changedFiles} 文件)· finder 预算 ${budget} · ${swarmStat}\n` +
    (plan ? `> Plan: \`${plan.name}\`${plan.path ? "" : "(未找到文件)"}\n` : "") +
    (diffTruncated ? `> ⚠️ diff 过大已截断至 ${MAX_DIFF} 字符\n` : "") +
    `\n> [!WARNING]\n> 本审核由多模型 agent 自动编排生成,**不可 100% 信赖**。请自行核对 file:line 与上下文再决定是否采纳。\n\n`;
  const tokenFooter =
    `\n\n---\n📊 **token 消耗**:总 **${num(grandTotal)}**` +
    (totalCost ? `(约 $${totalCost.toFixed(4)})` : "") +
    (perModel ? `\n> ${perModel}` : "") +
    `\n`;

  if (reviewText) {
    body = header + reviewText + tokenFooter;
  } else {
    body =
      header +
      `_orchestrator 未产出 review${queryErr ? `(query 异常: ${String(queryErr.message || queryErr).slice(0, 200)})` : ""},降级:仅给规模信息。请人工复核或重试。_\n` +
      tokenFooter;
  }

  writeFileSync("/tmp/review.md", body);
  if (DRY_RUN) {
    console.error("— REVIEW_DRY_RUN:不发评论,正文如下 —");
    console.log(body);
  } else {
    try {
      gh(`pr comment ${PR} --body-file /tmp/review.md`);
      console.error("✅ 已发布 review 评论");
    } catch (e) {
      console.error("发布评论失败:", e.message);
      console.error(body);
      process.exit(1);
    }
  }
}

const isEntry = import.meta.url === `file://${process.argv[1]}`;
if (isEntry) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
