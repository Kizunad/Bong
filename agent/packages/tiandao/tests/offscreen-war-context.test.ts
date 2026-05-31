import { describe, expect, it } from "vitest";
import type { NpcDeathV1 } from "@bong/schema";
import {
  CALAMITY_RECIPE,
  ERA_RECIPE,
  MUTATION_RECIPE,
  assembleContext,
  createContextInput,
  offscreenWarBlock,
} from "../src/context.js";
import { OFFSCREEN_WAR_REGION_CELL_SIZE } from "../src/offscreen-war-narration.js";
import { createTestWorldState } from "./support/fakes.js";

const CELL = OFFSCREEN_WAR_REGION_CELL_SIZE;

function posInCell(cx: number, cz: number): [number, number, number] {
  return [cx * CELL + CELL / 2, 64, cz * CELL + CELL / 2];
}

function combatDeath(overrides: Partial<NpcDeathV1> = {}): NpcDeathV1 {
  return {
    v: 1,
    kind: "npc_death",
    npc_id: "dormant:combat:a",
    archetype: "rogue",
    cause: "combat",
    faction_id: "attack",
    age_ticks: 10_000,
    max_age_ticks: 200_000,
    at_tick: 84_000,
    from_dormant_combat: true,
    pos: posInCell(-1, -1),
    ...overrides,
  };
}

function agingDeath(overrides: Partial<NpcDeathV1> = {}): NpcDeathV1 {
  return {
    v: 1,
    kind: "npc_death",
    npc_id: "npc:elder:1",
    archetype: "commoner",
    cause: "natural_aging",
    age_ticks: 200_000,
    max_age_ticks: 200_000,
    at_tick: 84_500,
    from_dormant_combat: false,
    ...overrides,
  };
}

describe("offscreenWarBlock — feeds aggregated war report into deduction context", () => {
  it("renders nothing when no death events present", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {});
    expect(offscreenWarBlock.render(input)).toBe("");
  });

  it("renders nothing when death events contain no offscreen combat deaths", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      npcDeathEvents: [agingDeath(), agingDeath({ npc_id: "npc:elder:2" })],
    });
    expect(offscreenWarBlock.render(input)).toBe("");
  });

  it("renders an anonymous war report covering only combat deaths (group_by region+faction)", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      npcDeathEvents: [
        combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(-1, -1), faction_id: "attack" }),
        combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(-1, -1), faction_id: "attack" }),
        combatDeath({ npc_id: "dormant:combat:3", pos: posInCell(-1, -1), faction_id: "defend" }),
        agingDeath({ npc_id: "npc:elder:1" }),
      ],
    });
    const text = offscreenWarBlock.render(input);
    expect(text).toContain("离屏散修消长");
    // 3 名 combat 战死（aging 不计）
    expect(text).toContain("3 名散修");
    // 北西方（cell -1,-1）方位锚点
    expect(text).toContain("北西方");
    // attack 组 2 人、defend 组 1 人，两条 tally 行
    expect(text).toContain("折损 2 人");
    expect(text).toContain("折损 1 人");
  });

  it("stays anonymous — never names a concrete sect, never leaks region cell index / faction id", () => {
    const text = offscreenWarBlock.render(
      createContextInput(createTestWorldState(), [], undefined, {
        npcDeathEvents: [
          combatDeath({ npc_id: "a", pos: posInCell(1, 1), faction_id: "attack" }),
          combatDeath({ npc_id: "b", pos: posInCell(-2, 3), faction_id: "defend" }),
        ],
      }),
    );
    for (const banned of ["玄岭", "断魂", "青云", "沧渊", "宗门"]) {
      expect(text.includes(banned), `must stay anonymous, leaked '${banned}': ${text}`).toBe(false);
    }
    // 内部分组键 / 坐标不能泄漏给天道
    expect(text).not.toContain("region:");
    expect(text).not.toMatch(/attack|defend|neutral/);
    // 明确散修群体涌现 framing，非组织化势力（且不含任何具名宗门 token）
    expect(text).toContain("散修");
    expect(text).toContain("非组织化势力");
  });

  it("treats pos=None combat deaths under an anonymous unknown locale", () => {
    const noPos = combatDeath({ npc_id: "dormant:combat:nopos" });
    delete (noPos as { pos?: [number, number, number] }).pos;
    const text = offscreenWarBlock.render(
      createContextInput(createTestWorldState(), [], undefined, { npcDeathEvents: [noPos] }),
    );
    expect(text).toContain("不知名的荒野");
    expect(text).toContain("1 名散修");
  });
});

describe("recipe wiring — offscreenWarBlock fed to 变化(mutation) / 演绎(era), not 灾劫(calamity)", () => {
  it("MUTATION_RECIPE includes offscreen_war block", () => {
    expect(MUTATION_RECIPE.blocks.some((b) => b.name === "offscreen_war")).toBe(true);
  });

  it("ERA_RECIPE includes offscreen_war block", () => {
    expect(ERA_RECIPE.blocks.some((b) => b.name === "offscreen_war")).toBe(true);
  });

  it("CALAMITY_RECIPE does NOT include offscreen_war block (per dossier落点)", () => {
    expect(CALAMITY_RECIPE.blocks.some((b) => b.name === "offscreen_war")).toBe(false);
  });

  it("assembleContext for mutation surfaces the war report when combat deaths present", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      agentName: "mutation",
      npcDeathEvents: [
        combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(-1, -1) }),
        combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(-1, -1) }),
      ],
    });
    const assembled = assembleContext(MUTATION_RECIPE, input);
    expect(assembled).toContain("离屏散修消长");
    expect(assembled).toContain("北西方");
  });

  it("assembleContext for era surfaces the war report when combat deaths present", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      agentName: "era",
      npcDeathEvents: [combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(1, 1) })],
    });
    const assembled = assembleContext(ERA_RECIPE, input);
    expect(assembled).toContain("离屏散修消长");
    expect(assembled).toContain("南东方");
  });

  it("assembleContext omits the block entirely when only aging deaths present", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      agentName: "mutation",
      npcDeathEvents: [agingDeath()],
    });
    const assembled = assembleContext(MUTATION_RECIPE, input);
    expect(assembled).not.toContain("离屏散修消长");
  });
});
