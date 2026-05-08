import { describe, expect, it } from "vitest";

import {
  meridianName,
  renderMeridianSeveredNarration,
  renderSeveredText,
} from "../src/meridian-severed-narration.js";
import type { MeridianSeveredEventV1 } from "@bong/schema";

const sample = (override: Partial<MeridianSeveredEventV1>): MeridianSeveredEventV1 => ({
  v: 1,
  type: "meridian_severed",
  entity_id: "player:abc",
  meridian_id: "Lung",
  source: "CombatWound",
  at_tick: 100,
  ...override,
});

describe("meridianName", () => {
  it("maps 12 正经 to canonical 中文 names", () => {
    expect(meridianName("Lung")).toBe("手太阴肺经");
    expect(meridianName("Heart")).toBe("手少阴心经");
    expect(meridianName("Liver")).toBe("足厥阴肝经");
    expect(meridianName("Kidney")).toBe("足少阴肾经");
  });

  it("maps 8 奇经 with proper 中文 names", () => {
    expect(meridianName("Ren")).toBe("任脉");
    expect(meridianName("Du")).toBe("督脉");
    expect(meridianName("YinWei")).toBe("阴维脉");
    expect(meridianName("YangQiao")).toBe("阳跷脉");
  });
});

describe("renderSeveredText (7 类来源)", () => {
  it("VoluntarySever 表达「自截而断 + 舍弃」", () => {
    const text = renderSeveredText("Lung", "VoluntarySever");
    expect(text).toContain("手太阴肺经");
    expect(text).toMatch(/自截|舍弃/);
  });

  it("BackfireOverload 表达反噬 + 真元逆流", () => {
    const text = renderSeveredText("Heart", "BackfireOverload");
    expect(text).toContain("手少阴心经");
    expect(text).toMatch(/反噬|逆流|撕扯/);
  });

  it("OverloadTear 表达强行调动超流量", () => {
    const text = renderSeveredText("LargeIntestine", "OverloadTear");
    expect(text).toContain("手阳明大肠经");
    expect(text).toMatch(/强行调动|代价|脉壁/);
  });

  it("CombatWound 表达战场重伤", () => {
    const text = renderSeveredText("Bladder", "CombatWound");
    expect(text).toContain("足太阳膀胱经");
    expect(text).toMatch(/搏杀|穿透|江湖/);
  });

  it("TribulationFail 表达天劫崩裂 + 降境之苦", () => {
    const text = renderSeveredText("Du", "TribulationFail");
    expect(text).toContain("督脉");
    expect(text).toMatch(/天劫|降境|崩裂/);
  });

  it("DuguDistortion 表达阴诡色慢慢蚀透", () => {
    const text = renderSeveredText("Liver", "DuguDistortion");
    expect(text).toContain("足厥阴肝经");
    expect(text).toMatch(/阴诡色|蚀透|药引/);
  });

  it("Other source 透传缘由文本", () => {
    const text = renderSeveredText("Chong", { Other: "上古遗物反噬" });
    expect(text).toContain("冲脉");
    expect(text).toContain("上古遗物反噬");
  });
});

describe("renderMeridianSeveredNarration", () => {
  it("returns null for non-meridian_severed events", () => {
    // @ts-expect-error 故意传错 type 测试守卫
    expect(renderMeridianSeveredNarration({ type: "other" })).toBeNull();
  });

  it("renders Narration with player scope + tick-keyed target", () => {
    const event = sample({ entity_id: "player:abc", meridian_id: "Lung", at_tick: 555 });
    const narr = renderMeridianSeveredNarration(event);
    expect(narr).not.toBeNull();
    expect(narr?.scope).toBe("player");
    expect(narr?.style).toBe("narration");
    expect(narr?.target).toContain("player:abc");
    expect(narr?.target).toContain("Lung");
    expect(narr?.target).toContain("555");
  });

  it("Narration text 包含经脉中文名", () => {
    const narr = renderMeridianSeveredNarration(sample({ meridian_id: "YinQiao" }));
    expect(narr?.text).toContain("阴跷脉");
  });

  it("不输出英文 enum 名（worldview §四:286 古意检测预防）", () => {
    const narr = renderMeridianSeveredNarration(sample({ meridian_id: "TripleEnergizer" }));
    // Narration 文本不应该出现英文 MeridianId
    expect(narr?.text).not.toMatch(/TripleEnergizer/);
    expect(narr?.text).not.toMatch(/CombatWound/);
  });

  it("不出现现代俚语 / 生理标签（古意检测红线）", () => {
    // worldview §四:286 + plan-narrative-political-v1 古意检测：narration 不应出现
    // 西医解剖名 / 现代俚语 / 生理标签（如「心率」「血压」「神经」「细胞」）
    const FORBIDDEN = ["心率", "血压", "神经", "细胞", "DNA", "ok", "Ok", "OK"];
    for (const meridian of ["Lung", "Heart", "Du", "Ren"] as const) {
      for (const source of [
        "VoluntarySever",
        "BackfireOverload",
        "OverloadTear",
        "CombatWound",
        "TribulationFail",
        "DuguDistortion",
      ] as const) {
        const text = renderSeveredText(meridian, source);
        for (const word of FORBIDDEN) {
          expect(text).not.toContain(word);
        }
      }
    }
  });
});
