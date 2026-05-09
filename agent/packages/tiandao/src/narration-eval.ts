import type { Narration, NarrationStyle } from "@bong/schema";
import { access, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

export const NARRATION_LOW_SCORE_THRESHOLD = 0.5;
export const NARRATION_LOW_SCORE_ANCHOR = "narration_low_score";
export const DEFAULT_NARRATION_REPORT_LIMIT = 50;

const MIN_NARRATION_LENGTH = 100;
const MAX_NARRATION_LENGTH = 200;
const TICK_LOG_PREFIX = "[tiandao:tick] ";
const MODERN_SLANG_RE =
  /(?:\b(?:ok|lol|bro|buff|nerf|gg|wtf|xp)\b|恭喜|注意|警告|小心|等级提升|哈哈|666|牛(?:啊|了)?|服了|离谱|刷怪|yyds|233)/iu;
const OMEN_RE = /(?:预兆|先兆|暗示|伏笔|将|欲|渐|未几|不日|下一轮|后势|后续|且看|宜早|将至|将起|或将)/u;
export const JIANGHU_VOICE_RE = /(?:江湖|传|道是|市井|山中|闻者|相传|有人道|外有声|画影|消息|传至|流传|耳闻)/u;
export const MODERN_POLITICAL_TERMS_BLACKLIST =
  /(?:政府|党派|选举|投票|民主|议会|总统|主席|内阁|联邦|国家|政权)/u;

const BASE_STYLE_KEYWORDS = [
  "诸",
  "众修",
  "天地",
  "灵脉",
  "云气",
  "山川",
  "草木",
  "今朝",
  "此间",
  "大势",
] as const;

const STYLE_KEYWORDS: Record<NarrationStyle, readonly string[]> = {
  system_warning: [
    "劫云",
    "雷",
    "天象",
    "因果",
    "戒",
    "警",
    "兽鸣",
    "风色",
    "压野",
    "杀伐",
  ],
  perception: [
    "灵气",
    "地脉",
    "山川",
    "草木",
    "云气",
    "风向",
    "潮",
    "呼吸",
    "流转",
    "异动",
  ],
  narration: ["天地", "灵脉", "草木", "山川", "云气", "众修", "将", "渐"],
  era_decree: [
    "天道",
    "昭告",
    "纪",
    "时代",
    "大势",
    "诸域",
    "此后",
    "众修",
    "宣告",
    "末法",
  ],
  political_jianghu: [
    "江湖",
    "传",
    "市井",
    "山中",
    "闻者",
    "相传",
    "画影",
    "消息",
  ],
};

const NARRATION_ISSUE_ORDER = [
  "length_out_of_range",
  "missing_omen",
  "modern_slang",
  "style_mismatch",
] as const;

const STYLE_ORDER: readonly NarrationStyle[] = [
  "system_warning",
  "perception",
  "narration",
  "era_decree",
  "political_jianghu",
];

const SCORE_BUCKETS = [
  { min: 0, max: 0.2, label: "0.00-0.19" },
  { min: 0.2, max: 0.4, label: "0.20-0.39" },
  { min: 0.4, max: 0.6, label: "0.40-0.59" },
  { min: 0.6, max: 0.8, label: "0.60-0.79" },
  { min: 0.8, max: 1.000_001, label: "0.80-1.00" },
] as const;

export type NarrationIssueCode = (typeof NARRATION_ISSUE_ORDER)[number];

export interface NarrationScore {
  lengthOk: boolean;
  hasOmen: boolean;
  noModernSlang: boolean;
  styleMatch: boolean;
  score: number;
}

export interface PoliticalNarrationContext {
  exposedIdentities?: Iterable<string>;
  unexposedIdentities?: Iterable<string>;
}

export interface PoliticalNarrationScore extends NarrationScore {
  hasJianghuVoice: boolean;
  noModernPoliticalTerms: boolean;
  anonymityOk: boolean;
}

export interface NarrationEvaluation extends NarrationScore {
  scope: Narration["scope"];
  target?: string;
  style: NarrationStyle;
  text: string;
  textLength: number;
  issues: NarrationIssueCode[];
}

export interface NarrationReport {
  totalSamples: number;
  windowSize: number;
  averageScore: number;
  lowScoreCount: number;
  lowScoreThreshold: number;
  scoreDistribution: Array<{ label: string; count: number }>;
  issuePatterns: Array<{ issue: NarrationIssueCode; count: number }>;
  styleBreakdown: Array<{ style: NarrationStyle; count: number; averageScore: number }>;
}

export interface NarrationEvalCliIo {
  cwd?: string;
  writeStdout?: (text: string) => void;
  writeStderr?: (text: string) => void;
}

interface ParsedCliArgs {
  limit: number;
  files: string[];
  help: boolean;
}

interface TickMetadataLike {
  sourceTick: number;
  correlationId: string;
}

interface NarrationTelemetryLike {
  narrationScores?: unknown;
}

export function scoreNarration(text: string, style: NarrationStyle): NarrationScore {
  const textLength = countNarrationCharacters(text);
  const lengthOk = textLength >= MIN_NARRATION_LENGTH && textLength <= MAX_NARRATION_LENGTH;
  const hasOmen = OMEN_RE.test(text);
  const noModernSlang = !MODERN_SLANG_RE.test(text);
  const styleMatch = checkStyleKeywords(text, style);
  const score = roundScore(
    (lengthOk ? 0.25 : 0) +
      (hasOmen ? 0.3 : 0) +
      (noModernSlang ? 0.2 : 0) +
      (styleMatch ? 0.25 : 0),
  );

  return {
    lengthOk,
    hasOmen,
    noModernSlang,
    styleMatch,
    score,
  };
}

export function checkAnonymityViolation(
  narration: string,
  context: PoliticalNarrationContext = {},
): boolean {
  const exposed = normalizeIdentitySet(context.exposedIdentities);
  const unexposed = normalizeIdentitySet(context.unexposedIdentities);

  for (const name of unexposed) {
    if (name.length > 0 && narration.includes(name) && !exposed.has(name)) {
      return true;
    }
  }
  return false;
}

export function scorePoliticalNarration(
  narration: string,
  context: PoliticalNarrationContext = {},
): PoliticalNarrationScore {
  const baseScore = scoreNarration(narration, "political_jianghu");
  const hasJianghuVoice = JIANGHU_VOICE_RE.test(narration);
  const noModernPoliticalTerms = !MODERN_POLITICAL_TERMS_BLACKLIST.test(narration);
  const anonymityOk = !checkAnonymityViolation(narration, context);
  const penalty =
    (hasJianghuVoice ? 0 : 0.3) +
    (noModernPoliticalTerms ? 0 : 0.5) +
    (anonymityOk ? 0 : 0.6);

  return {
    ...baseScore,
    hasJianghuVoice,
    noModernPoliticalTerms,
    anonymityOk,
    score: roundScore(baseScore.score - penalty),
  };
}

export function evaluateNarration(narration: Narration): NarrationEvaluation {
  const score = scoreNarration(narration.text, narration.style);

  return {
    ...score,
    scope: narration.scope,
    target: narration.target,
    style: narration.style,
    text: narration.text,
    textLength: countNarrationCharacters(narration.text),
    issues: collectNarrationIssues(score),
  };
}

export function evaluateNarrations(narrations: Narration[]): NarrationEvaluation[] {
  return narrations.map((narration) => evaluateNarration(narration));
}

export function summarizeNarrationAverage(evaluations: NarrationEvaluation[]): number {
  if (evaluations.length === 0) {
    return 0;
  }

  const total = evaluations.reduce((sum, evaluation) => sum + evaluation.score, 0);
  return roundScore(total / evaluations.length);
}

export function formatNarrationLowScoreWarning(args: {
  evaluation: NarrationEvaluation;
  metadata: TickMetadataLike;
  index: number;
}): string {
  const { evaluation, metadata, index } = args;
  const issues = evaluation.issues.length > 0 ? evaluation.issues.join(",") : "none";
  const target = evaluation.target ? ` target=${sanitizeLogToken(evaluation.target)}` : "";

  return (
    `[tiandao] ⚠️ ${NARRATION_LOW_SCORE_ANCHOR}` +
    ` tick=${metadata.sourceTick}` +
    ` correlation_id=${metadata.correlationId}` +
    ` index=${index}` +
    ` scope=${evaluation.scope}` +
    target +
    ` style=${evaluation.style}` +
    ` score=${evaluation.score.toFixed(3)}` +
    ` issues=${issues}`
  );
}

export function parseNarrationEvaluationsFromText(logText: string): NarrationEvaluation[] {
  const evaluations: NarrationEvaluation[] = [];

  for (const line of logText.split(/\r?\n/u)) {
    if (!line.startsWith(TICK_LOG_PREFIX)) {
      continue;
    }

    const payload = line.slice(TICK_LOG_PREFIX.length).trim();
    if (payload.length === 0) {
      continue;
    }

    try {
      const parsed = JSON.parse(payload) as NarrationTelemetryLike;
      if (!Array.isArray(parsed.narrationScores)) {
        continue;
      }

      for (const candidate of parsed.narrationScores) {
        const normalized = normalizeNarrationEvaluation(candidate);
        if (normalized) {
          evaluations.push(normalized);
        }
      }
    } catch {}
  }

  return evaluations;
}

export function buildNarrationReport(
  evaluations: NarrationEvaluation[],
  options: { limit?: number } = {},
): NarrationReport {
  const windowSize = Math.max(1, Math.trunc(options.limit ?? DEFAULT_NARRATION_REPORT_LIMIT));
  const window = evaluations.slice(-windowSize);

  const scoreDistribution = SCORE_BUCKETS.map((bucket) => ({
    label: bucket.label,
    count: window.filter((entry) => entry.score >= bucket.min && entry.score < bucket.max).length,
  }));
  const issuePatterns = NARRATION_ISSUE_ORDER.map((issue) => ({
    issue,
    count: window.filter((entry) => entry.issues.includes(issue)).length,
  }));
  const styleBreakdown = STYLE_ORDER.map((style) => {
    const matching = window.filter((entry) => entry.style === style);
    return {
      style,
      count: matching.length,
      averageScore: summarizeNarrationAverage(matching),
    };
  }).filter((entry) => entry.count > 0);

  return {
    totalSamples: window.length,
    windowSize,
    averageScore: summarizeNarrationAverage(window),
    lowScoreCount: window.filter((entry) => entry.score < NARRATION_LOW_SCORE_THRESHOLD).length,
    lowScoreThreshold: NARRATION_LOW_SCORE_THRESHOLD,
    scoreDistribution,
    issuePatterns,
    styleBreakdown,
  };
}

export function renderNarrationReport(report: NarrationReport): string {
  const lines: string[] = [];
  lines.push("Narration evaluation report");
  lines.push(
    `samples=${report.totalSamples} window=${report.windowSize} avg_score=${report.averageScore.toFixed(3)} low_score=${report.lowScoreCount} threshold=${report.lowScoreThreshold.toFixed(2)}`,
  );
  lines.push("");
  lines.push("score_distribution");

  for (const bucket of report.scoreDistribution) {
    lines.push(`${bucket.label} | ${"#".repeat(bucket.count)} (${bucket.count})`);
  }

  lines.push("");
  lines.push("issue_patterns");
  if (report.issuePatterns.every((entry) => entry.count === 0)) {
    lines.push("(none)");
  } else {
    for (const issue of report.issuePatterns) {
      lines.push(`${issue.issue.padEnd(19, " ")} | ${"#".repeat(issue.count)} (${issue.count})`);
    }
  }

  lines.push("");
  lines.push("style_breakdown");
  if (report.styleBreakdown.length === 0) {
    lines.push("(none)");
  } else {
    for (const style of report.styleBreakdown) {
      lines.push(
        `${style.style.padEnd(14, " ")} | count=${String(style.count).padEnd(2, " ")} avg=${style.averageScore.toFixed(3)}`,
      );
    }
  }

  return `${lines.join("\n")}\n`;
}

export async function runNarrationEvalCli(
  args: string[],
  io: NarrationEvalCliIo = {},
): Promise<number> {
  const cwd = io.cwd ?? process.cwd();
  const writeStdout = io.writeStdout ?? ((text: string) => process.stdout.write(text));
  const writeStderr = io.writeStderr ?? ((text: string) => process.stderr.write(text));
  const parsed = parseCliArgs(args);

  if (parsed.help) {
    writeStdout(renderCliUsage());
    return 0;
  }

  const files = parsed.files.length > 0 ? parsed.files.map((file) => resolve(cwd, file)) : await findDefaultLogFiles(cwd);
  if (files.length === 0) {
    writeStderr(
      "No local tiandao telemetry log found. Pass --file <path> or place a log at ./tiandao.log, ./tiandao.jsonl, ./data/tiandao.log, or ./data/tiandao.jsonl.\n",
    );
    return 1;
  }

  const evaluations = await readNarrationEvaluationsFromFiles(files);
  const report = buildNarrationReport(evaluations, { limit: parsed.limit });
  writeStdout(renderNarrationReport(report));
  return 0;
}

function checkStyleKeywords(text: string, style: NarrationStyle): boolean {
  const baseHits = countKeywordHits(text, BASE_STYLE_KEYWORDS);
  const styleHits = countKeywordHits(text, STYLE_KEYWORDS[style]);

  if (style === "era_decree") {
    return (text.includes("天道") || text.includes("昭告") || text.includes("纪")) &&
      baseHits + styleHits >= 2;
  }

  return baseHits >= 1 && styleHits >= 1;
}

function countKeywordHits(text: string, keywords: readonly string[]): number {
  return keywords.reduce((count, keyword) => count + (text.includes(keyword) ? 1 : 0), 0);
}

function collectNarrationIssues(score: NarrationScore): NarrationIssueCode[] {
  const issues: NarrationIssueCode[] = [];
  if (!score.lengthOk) {
    issues.push("length_out_of_range");
  }
  if (!score.hasOmen) {
    issues.push("missing_omen");
  }
  if (!score.noModernSlang) {
    issues.push("modern_slang");
  }
  if (!score.styleMatch) {
    issues.push("style_mismatch");
  }
  return issues;
}

function countNarrationCharacters(text: string): number {
  return Array.from(text).filter((character) => !/\s/u.test(character)).length;
}

function roundScore(value: number): number {
  return Math.round(clamp(value, 0, 1) * 1000) / 1000;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function sanitizeLogToken(value: string): string {
  return value.replace(/\s+/gu, "_");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNarrationStyle(value: unknown): value is NarrationStyle {
  return typeof value === "string" && STYLE_ORDER.includes(value as NarrationStyle);
}

function isNarrationScope(value: unknown): value is Narration["scope"] {
  return value === "broadcast" || value === "zone" || value === "player";
}

function isNarrationIssueCode(value: unknown): value is NarrationIssueCode {
  return typeof value === "string" && NARRATION_ISSUE_ORDER.includes(value as NarrationIssueCode);
}

function normalizeNarrationEvaluation(candidate: unknown): NarrationEvaluation | null {
  if (!isRecord(candidate)) {
    return null;
  }

  const text = candidate.text;
  const style = candidate.style;
  const scope = candidate.scope;

  if (typeof text !== "string" || !isNarrationStyle(style) || !isNarrationScope(scope)) {
    return null;
  }

  const rescored = scoreNarration(text, style);
  const issues = Array.isArray(candidate.issues)
    ? candidate.issues.filter((issue): issue is NarrationIssueCode => isNarrationIssueCode(issue))
    : collectNarrationIssues(rescored);

  return {
    lengthOk: typeof candidate.lengthOk === "boolean" ? candidate.lengthOk : rescored.lengthOk,
    hasOmen: typeof candidate.hasOmen === "boolean" ? candidate.hasOmen : rescored.hasOmen,
    noModernSlang:
      typeof candidate.noModernSlang === "boolean" ? candidate.noModernSlang : rescored.noModernSlang,
    styleMatch: typeof candidate.styleMatch === "boolean" ? candidate.styleMatch : rescored.styleMatch,
    score: typeof candidate.score === "number" ? roundScore(candidate.score) : rescored.score,
    scope,
    target: typeof candidate.target === "string" ? candidate.target : undefined,
    style,
    text,
    textLength: typeof candidate.textLength === "number" ? candidate.textLength : countNarrationCharacters(text),
    issues,
  };
}

function normalizeIdentitySet(values: Iterable<string> | undefined): Set<string> {
  const normalized = new Set<string>();
  if (!values) return normalized;
  for (const value of values) {
    const trimmed = value.trim();
    if (trimmed) normalized.add(trimmed);
  }
  return normalized;
}

async function readNarrationEvaluationsFromFiles(filePaths: string[]): Promise<NarrationEvaluation[]> {
  const evaluations: NarrationEvaluation[] = [];

  for (const filePath of filePaths) {
    const contents = await readFile(filePath, "utf8");
    evaluations.push(...parseNarrationEvaluationsFromText(contents));
  }

  return evaluations;
}

function parseCliArgs(args: string[]): ParsedCliArgs {
  const files: string[] = [];
  let limit = DEFAULT_NARRATION_REPORT_LIMIT;
  let help = false;

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];

    if (arg === "--help" || arg === "-h") {
      help = true;
      continue;
    }

    if (arg === "--file") {
      const file = args[index + 1];
      if (!file) {
        throw new Error("Missing value for --file");
      }
      files.push(file);
      index += 1;
      continue;
    }

    if (arg === "--limit") {
      const rawLimit = args[index + 1];
      if (!rawLimit) {
        throw new Error("Missing value for --limit");
      }
      const parsedLimit = Number.parseInt(rawLimit, 10);
      if (!Number.isFinite(parsedLimit) || parsedLimit <= 0) {
        throw new Error(`Invalid --limit value: ${rawLimit}`);
      }
      limit = parsedLimit;
      index += 1;
      continue;
    }

    files.push(arg);
  }

  return {
    limit,
    files,
    help,
  };
}

async function findDefaultLogFiles(cwd: string): Promise<string[]> {
  const candidates = [
    resolve(cwd, "tiandao.log"),
    resolve(cwd, "tiandao.jsonl"),
    resolve(cwd, "data/tiandao.log"),
    resolve(cwd, "data/tiandao.jsonl"),
  ];
  const existing: string[] = [];

  for (const candidate of candidates) {
    if (await fileExists(candidate)) {
      existing.push(candidate);
    }
  }

  return existing;
}

async function fileExists(filePath: string): Promise<boolean> {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

function renderCliUsage(): string {
  return `Usage: npm run eval-narrations -- [--file <path>] [--limit <n>]
Reads [tiandao:tick] JSON log lines and prints an ASCII narration quality report.
`;
}

const __filename = fileURLToPath(import.meta.url);
if (process.argv[1] === __filename) {
  runNarrationEvalCli(process.argv.slice(2)).then((exitCode) => {
    if (exitCode !== 0) {
      process.exit(exitCode);
    }
  }).catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`[tiandao] narration eval failed: ${message}\n`);
    process.exit(1);
  });
}
