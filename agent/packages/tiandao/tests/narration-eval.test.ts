import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Narration } from "@bong/schema";
import { describe, expect, it, vi } from "vitest";
import {
  NARRATION_LOW_SCORE_ANCHOR,
  evaluateNarration,
  renderNarrationReport,
  runNarrationEvalCli,
  scoreNarration,
  buildNarrationReport,
  parseNarrationEvaluationsFromText,
  checkAnonymityViolation,
  scorePoliticalNarration,
} from "../src/narration-eval.js";
import { DEFAULT_MODEL, runTick } from "../src/runtime.js";
import { createZeroErrorBreakdown, type TelemetrySink, type TickMetrics } from "../src/telemetry.js";
import { FakeAgent, createTestWorldState } from "./support/fakes.js";

function padNarrationWindow(seed: string, targetLength = 130): string {
  let output = seed;
  while (Array.from(output).length < targetLength) {
    output += seed;
  }
  return Array.from(output).slice(0, targetLength).join("");
}

function buildMetrics(tick: number, narrations: Narration[]): TickMetrics {
  const narrationScores = narrations.map((narration) => evaluateNarration(narration));
  return {
    tick,
    timestamp: 1_700_000_000_000 + tick,
    durationMs: 100,
    agentResults: [],
    mergedCommandCount: 0,
    mergedNarrationCount: narrations.length,
    chatSignalCount: 0,
    eraChanged: false,
    errorBreakdown: createZeroErrorBreakdown(),
    staleStateSkipped: false,
    narrationScores,
    narrationLowScoreCount: narrationScores.filter((entry) => entry.score < 0.5).length,
    narrationAverageScore:
      narrationScores.length > 0
        ? narrationScores.reduce((sum, entry) => sum + entry.score, 0) / narrationScores.length
        : 0,
  };
}

describe("narration evaluation", () => {
  it("scores strong semi-classical warning narrations highly", () => {
    const text = padNarrationWindow(
      "劫云低垂，因果未歇，北岭草木皆伏，风色如刃；皆因强者久据灵脉、杀伐不息，故使雷意潜行于野。今朝兽鸣先乱，暮后更见电痕游走，若众修仍不知敛，下一轮天象将至，宜早收锋守心。",
    );

    const score = scoreNarration(text, "system_warning");

    expect(score.lengthOk).toBe(true);
    expect(score.hasOmen).toBe(true);
    expect(score.noModernSlang).toBe(true);
    expect(score.styleMatch).toBe(true);
    expect(score.score).toBeGreaterThanOrEqual(0.8);
  });

  it("penalizes short slang-heavy narrations without omen", () => {
    const score = scoreNarration("哈哈666 OK bro，今晚继续刷怪，灵气 buff 了，太牛了。", "system_warning");

    expect(score.lengthOk).toBe(false);
    expect(score.hasOmen).toBe(false);
    expect(score.noModernSlang).toBe(false);
    expect(score.styleMatch).toBe(false);
    expect(score.score).toBeLessThan(0.5);
  });

  it("penalizes worldview-forbidden game prompt wording", () => {
    const score = scoreNarration(
      padNarrationWindow("恭喜！注意！警告！小心，前方有危险怪物，xp 与等级提升都已触发。"),
      "system_warning",
    );

    expect(score.noModernSlang).toBe(false);
    expect(score.score).toBeLessThan(0.8);
  });

  it("renders deterministic ASCII report from telemetry tick logs", async () => {
    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-narration-eval-"));
    const logPath = join(tempDir, "tiandao.log");
    const output: string[] = [];
    const errors: string[] = [];

    const strongEra: Narration = {
      scope: "broadcast",
      style: "era_decree",
      text: padNarrationWindow(
        "天道昭告：赤霄纪之所以将立，皆因诸域强弱失衡已久，今朝灵机如潮退、劫云低垂，众修当知大势将迁；若仍竞逐旧路，后势必更烈，宜早观变而守正。",
      ),
    };
    const weakWarning: Narration = {
      scope: "zone",
      target: "starter_zone",
      style: "system_warning",
      text: "哈哈666 OK bro，今晚继续刷怪，太牛了。",
    };

    try {
      const metrics = buildMetrics(123, [strongEra, weakWarning]);
      await writeFile(logPath, `[tiandao:tick] ${JSON.stringify(metrics)}\n`, "utf8");

      const exitCode = await runNarrationEvalCli(["--file", logPath, "--limit", "50"], {
        cwd: tempDir,
        writeStdout: (text) => output.push(text),
        writeStderr: (text) => errors.push(text),
      });

      expect(exitCode).toBe(0);
      expect(errors).toEqual([]);
      expect(output.join("")).toContain("Narration evaluation report");
      expect(output.join("")).toContain("score_distribution");
      expect(output.join("")).toContain("issue_patterns");
      expect(output.join("")).toContain("style_breakdown");
      expect(output.join("")).toContain("modern_slang");
      expect(output.join("")).toContain("0.80-1.00 | # (1)");
    } finally {
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  it("scores merged narrations in runTick telemetry without mutating publish payloads", async () => {
    const lowNarration: Narration = {
      scope: "zone",
      target: "starter_zone",
      style: "system_warning",
      text: "哈哈666 OK bro，今晚继续刷怪，太牛了。",
    };
    const logger = { log: vi.fn(), error: vi.fn() };
    const publishNarrations = vi.fn(async () => {});
    const captured: { metrics: TickMetrics | null } = { metrics: null };
    const telemetrySink: TelemetrySink = {
      async recordTick(metrics) {
        captured.metrics = metrics;
      },
      async flush() {},
    };

    const result = await runTick(createTestWorldState(), {
      agents: [new FakeAgent("calamity", { commands: [], narrations: [lowNarration], reasoning: "low narration" })],
      llmClient: {
        chat: vi.fn(async () => ({
          content: "{}",
          durationMs: 0,
          requestId: null,
          model: DEFAULT_MODEL,
        })),
      },
      model: DEFAULT_MODEL,
      publishCommands: vi.fn(async () => {}),
      publishNarrations,
      telemetrySink,
      logger,
    });

    expect(publishNarrations).toHaveBeenCalledWith({
      narrations: [lowNarration],
      metadata: {
        sourceTick: 123,
        correlationId: "tiandao-tick-123",
      },
    });
    expect(result.metrics.narrationLowScoreCount).toBe(1);
    expect(result.metrics.narrationScores).toHaveLength(1);
    expect(result.metrics.narrationScores?.[0]).toEqual(
      expect.objectContaining({
        text: lowNarration.text,
        style: "system_warning",
        score: expect.any(Number),
        noModernSlang: false,
      }),
    );
    expect(captured.metrics?.narrationScores?.[0]?.text).toBe(lowNarration.text);
    expect(
      logger.log.mock.calls.some(
        ([message]) => typeof message === "string" && message.includes(NARRATION_LOW_SCORE_ANCHOR),
      ),
    ).toBe(true);
  });

  it("parses telemetry lines into reportable evaluation entries", () => {
    const narration: Narration = {
      scope: "zone",
      target: "starter_zone",
      style: "perception",
      text: padNarrationWindow(
        "山川气脉今朝微偏，皆因西麓灵气久滞，故使草木先寒、云气倒卷；若此势仍渐深，下一轮地脉将再移半分，行旅宜早察风向而定步。",
      ),
    };
    const text = `[tiandao:tick] ${JSON.stringify(buildMetrics(1, [narration]))}\n`;

    const entries = parseNarrationEvaluationsFromText(text);
    const report = buildNarrationReport(entries);
    const rendered = renderNarrationReport(report);

    expect(entries).toHaveLength(1);
    expect(entries[0]).toEqual(
      expect.objectContaining({
        style: "perception",
        scope: "zone",
        target: "starter_zone",
      }),
    );
    expect(rendered).toContain("samples=1");
    expect(rendered).toContain("perception");
  });

  it("scores political narration with jianghu voice highly", () => {
    const text = padNarrationWindow(
      "江湖有传，血谷旧怨又添一笔，市井只说两名修士各自收刀入袖；云气渐低，后势未明，旁人听罢只把灯挑暗。",
    );

    const score = scorePoliticalNarration(text, {
      unexposedIdentities: ["玄锋"],
    });

    expect(score.hasJianghuVoice).toBe(true);
    expect(score.noModernPoliticalTerms).toBe(true);
    expect(score.anonymityOk).toBe(true);
    expect(score.score).toBeGreaterThanOrEqual(0.8);
  });

  it("penalizes political narration without jianghu voice", () => {
    const strong = padNarrationWindow(
      "江湖有传，山川气脉今朝微偏，众修旧账渐深，后势将起；闻者不问名姓，只知灯下又多一行血字。",
    );
    const plain = padNarrationWindow(
      "山川气脉今朝微偏，众修旧账渐深，后势将起；旁人不问名姓，只知灯下又多一行血字。",
    );

    expect(scorePoliticalNarration(strong).score - scorePoliticalNarration(plain).score).toBeGreaterThanOrEqual(0.3);
  });

  it("penalizes modern political terms in political narration", () => {
    const text = padNarrationWindow(
      "江湖有传，某修士在山中建立政府与议会，众修将以投票定夺旧怨；市井闻者皆笑，此事后势未明。",
    );

    const score = scorePoliticalNarration(text);

    expect(score.noModernPoliticalTerms).toBe(false);
    expect(score.score).toBeLessThan(0.6);
  });

  it("penalizes naming unexposed identities but allows exposed names", () => {
    const text = padNarrationWindow(
      "江湖有传，玄锋之名已过诸渊，市井将把旧账添入灯下；后势未明，闻者只问此名何时再现。",
    );

    expect(checkAnonymityViolation(text, { unexposedIdentities: ["玄锋"] })).toBe(true);
    expect(
      scorePoliticalNarration(text, {
        exposedIdentities: ["玄锋"],
        unexposedIdentities: ["玄锋"],
      }).anonymityOk,
    ).toBe(true);
    expect(
      scorePoliticalNarration(text, {
        unexposedIdentities: ["玄锋"],
      }).score,
    ).toBeLessThan(
      scorePoliticalNarration(text, {
        exposedIdentities: ["玄锋"],
        unexposedIdentities: ["玄锋"],
      }).score,
    );
  });

  it("does not treat exposed longer names as unexposed short-name leaks", () => {
    const text = padNarrationWindow(
      "江湖有传，玄锋子之名已过诸渊，市井将把旧账添入灯下；后势未明，闻者只问此名何时再现。",
    );

    expect(
      checkAnonymityViolation(text, {
        exposedIdentities: ["玄锋子"],
        unexposedIdentities: ["玄锋"],
      }),
    ).toBe(false);
  });
});
