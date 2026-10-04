import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Value } from "@sinclair/typebox/value";
import { describe, expect, it } from "vitest";

import {
  ContainerOpenRequestV1,
  CLIENT_REQUEST_REGISTRY,
  CLIENT_REQUEST_TYPE_SET,
  ClientRequestV1,
  GiveDanToElderRequestV1,
  SupplyCoffinOpenRequestV1,
  WorkbenchOpenRequestV1,
} from "../src/client-request.js";
import { validate } from "../src/validate.js";

const repositoryRoot = join(fileURLToPath(new URL(".", import.meta.url)), "..", "..", "..", "..");

function toSnakeCase(name: string): string {
  return name.replace(/([A-Z])/g, (_, letter: string, offset: number) => {
    return `${offset === 0 ? "" : "_"}${letter.toLowerCase()}`;
  });
}

function rustClientRequestTypes(): Set<string> {
  const source = readFileSync(
    join(repositoryRoot, "server/src/schema/client_request.rs"),
    "utf8",
  );
  const body = source.match(
    /pub enum ClientRequestV1 \{([\s\S]*?)\n\}\n\nimpl ClientRequestV1/,
  )?.[1];
  if (!body) {
    throw new Error("ClientRequestV1 enum body is missing from the Rust schema");
  }

  return new Set(
    body
      .split("\n")
      .map((line) => line.match(/^\s{4}([A-Z][A-Za-z0-9_]*)\s*\{/))
      .filter((match): match is RegExpMatchArray => match !== null)
      .map((match) => toSnakeCase(match[1])),
  );
}

function protoClientRequestTypes(): Set<string> {
  const source = readFileSync(join(repositoryRoot, "proto/bong/envelope.proto"), "utf8");
  const body = source.match(
    /message ClientRequestEnvelope \{([\s\S]*?)\n\}/,
  )?.[1];
  if (!body) {
    throw new Error("ClientRequestEnvelope declaration is missing from protobuf");
  }

  return new Set(
    body
      .split("\n")
      .map((line) => line.match(/^\s+[A-Za-z0-9_]+\s+([a-z0-9_]+)\s*=\s*\d+;/))
      .filter((match): match is RegExpMatchArray => match !== null)
      .map((match) => match[1]),
  );
}

const POSITIVE_SAMPLE_OVERRIDES: Record<string, Record<string, unknown>> = {
  inventory_move_intent: {
    v: 1,
    type: "inventory_move_intent",
    instance_id: 7,
    from: { kind: "container", container_id: "main_pack", row: 0, col: 0 },
    to: { kind: "container", container_id: "main_pack", row: 0, col: 1 },
  },
  inventory_discard_item: {
    v: 1,
    type: "inventory_discard_item",
    instance_id: 7,
    from: { kind: "container", container_id: "main_pack", row: 0, col: 0 },
  },
  drop_weapon_intent: {
    v: 1,
    type: "drop_weapon_intent",
    instance_id: 7,
    from: { kind: "equip", slot: "main_hand", state: "held" },
  },
  external_container_move: {
    v: 1,
    type: "external_container_move",
    session_id: 11,
    instance_id: 7,
    from: { kind: "container", container_id: "main_pack", row: 0, col: 0 },
    to: { kind: "container", container_id: "main_pack", row: 0, col: 1 },
  },
  npc_dialogue_choice: {
    v: 1,
    type: "npc_dialogue_choice",
    npc_entity_id: 42,
    option_id: "greet",
  },
  npc_trade_request: {
    v: 1,
    type: "npc_trade_request",
    npc_entity_id: 42,
    requested_item_id: "herb_knife",
  },
};

describe("C2S registry contract", () => {
  it("derives one unique live type set from the current Rust and protobuf registries", () => {
    const registryTypes = new Set(CLIENT_REQUEST_TYPE_SET);
    const rustTypes = rustClientRequestTypes();
    const protoTypes = protoClientRequestTypes();

    expect(CLIENT_REQUEST_REGISTRY).toHaveLength(102);
    expect(registryTypes.size).toBe(CLIENT_REQUEST_REGISTRY.length);
    expect(registryTypes).toEqual(rustTypes);

    // BlockPickerGive and AgentUiResponse are JSON-only C2S paths; all remaining
    // live request types must have a protobuf envelope field.
    expect(new Set([...rustTypes].filter((type) => !protoTypes.has(type)))).toEqual(
      new Set(["agent_ui_response", "block_picker_give"]),
    );
    expect(new Set([...protoTypes].filter((type) => !rustTypes.has(type)))).toEqual(new Set());
  });

  for (const { type, schema } of CLIENT_REQUEST_REGISTRY) {
    it(`${type} has a positive and negative sample`, () => {
      const positive = POSITIVE_SAMPLE_OVERRIDES[type] ?? Value.Create(schema);
      const accepted = validate(schema, positive);
      expect(
        accepted.ok,
        `${type} generated positive sample should validate: ${accepted.errors.join("; ")}`,
      ).toBe(true);

      const negative = { ...(positive as Record<string, unknown>), type: `${type}_unknown` };
      const rejected = validate(ClientRequestV1, negative);
      expect(
        rejected.ok,
        `${type} unknown discriminator should be rejected as a negative sample`,
      ).toBe(false);
    });
  }
});

describe("entity-based C2S request bounds", () => {
  const i32Minimum = -2_147_483_648;
  const i32Maximum = 2_147_483_647;
  const cases = [
    {
      type: "supply_coffin_open",
      schema: SupplyCoffinOpenRequestV1,
      field: "entity_id",
      base: { v: 1, type: "supply_coffin_open" },
    },
    {
      type: "container_open",
      schema: ContainerOpenRequestV1,
      field: "entity_id",
      base: { v: 1, type: "container_open" },
    },
    {
      type: "workbench_open",
      schema: WorkbenchOpenRequestV1,
      field: "entity_id",
      base: { v: 1, type: "workbench_open" },
    },
    {
      type: "give_dan_to_elder",
      schema: GiveDanToElderRequestV1,
      field: "elder_entity_id",
      base: { v: 1, type: "give_dan_to_elder", pill_instance_id: 1 },
    },
  ] as const;

  for (const { type, schema, field, base } of cases) {
    it(`${type} accepts the Rust i32 bounds and rejects overflow`, () => {
      expect(validate(schema, { ...base, [field]: i32Minimum }).ok).toBe(true);
      expect(validate(schema, { ...base, [field]: i32Maximum }).ok).toBe(true);
      expect(validate(schema, { ...base, [field]: i32Minimum - 1 }).ok).toBe(false);
      expect(validate(schema, { ...base, [field]: i32Maximum + 1 }).ok).toBe(false);
    });
  }
});
