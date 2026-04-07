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
    }
  });

  it("requires era prompt to emit structured era metadata", () => {
    const content = readFileSync(resolve(skillsDir, "era.md"), "utf-8");

    expect(content).toContain('"target": "全局"');
    expect(content).toContain("era_name");
    expect(content).toContain("global_effect");
    expect(content).toContain("danger_level_delta");
  });
});
