import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const skillsDir = resolve(import.meta.dirname, "../src/skills");

describe("tiandao skill prompts", () => {
  it("requires semi-classical narration, 100-200 chars, omen, and JSON-only output", () => {
    for (const skillFile of ["era.md", "calamity.md", "mutation.md"]) {
      const content = readFileSync(resolve(skillsDir, skillFile), "utf-8");

      expect(content).toContain("半文言半白话");
      expect(content).toMatch(/100[–-]200/);
      expect(content).toMatch(/预兆|伏笔/);
      expect(content).toContain("纯 JSON");
      expect(content).toContain("合法 JSON 对象");
      expect(content).toContain("只读");
      expect(content).toContain("预算");
    }
  });

  it("requires era prompt to emit structured era metadata", () => {
    const content = readFileSync(resolve(skillsDir, "era.md"), "utf-8");

    expect(content).toContain('"target": "全局"');
    expect(content).toContain("era_name");
    expect(content).toContain("global_effect");
    expect(content).toContain("danger_level_delta");
  });

  it("documents optional read-only tool use in each skill prompt", () => {
    const calamity = readFileSync(resolve(skillsDir, "calamity.md"), "utf-8");
    const mutation = readFileSync(resolve(skillsDir, "mutation.md"), "utf-8");
    const era = readFileSync(resolve(skillsDir, "era.md"), "utf-8");

    expect(calamity).toContain("工具是可选的");
    expect(calamity).toContain("query-player");
    expect(calamity).toContain("query-player-skill-milestones");
    expect(calamity).toContain("list-active-events");
    expect(calamity).toContain("query-rat-density");
    expect(mutation).toContain("工具是可选的");
    expect(calamity).toContain("query-player");
    expect(calamity).toContain("query-player-skill-milestones");
    expect(calamity).toContain("list-active-events");
    expect(calamity).toContain("query-rat-density");
    expect(mutation).toContain("query-zone-history");
    expect(era).toContain("默认不使用工具");
    expect(era).toContain("默认无工具");
  });

  it("documents ecology pseudo vein decision rules", () => {
    const ecology = readFileSync(resolve(skillsDir, "ecology.md"), "utf-8");
    const calamity = readFileSync(resolve(skillsDir, "calamity.md"), "utf-8");

    expect(ecology).toContain('params.event = "pseudo_vein"');
    expect(ecology).toContain("玩家密度 > 3");
    expect(ecology).toContain("灵气消耗率 > 0.02/tick");
    expect(ecology).toContain("汐转期");
    expect(calamity).toContain("伪灵脉(pseudo_vein)");
    expect(calamity).toContain("引导分流和加速生态反馈");
  });

  it("loads political jianghu prompt with anonymity and blacklist rules", () => {
    const political = readFileSync(resolve(skillsDir, "political.md"), "utf-8");

    expect(political).toContain("江湖传声筒");
    expect(political).toContain("political_jianghu");
    expect(political).toContain("匿名约束");
    expect(political).toContain("政府");
    expect(political).toContain("纯 JSON");
  });
});
