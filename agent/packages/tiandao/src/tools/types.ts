import type { WorldStateV1 } from "@bong/schema";
import { WorldModel, type WorldModelSnapshot } from "../world-model.js";

export interface ToolContext {
  latestState: Readonly<WorldStateV1>;
  worldModel: Readonly<WorldModelSnapshot>;
}

export interface AgentTool<TArgs = unknown, TResult = unknown> {
  name: string;
  description: string;
  readonly: true;
  parameters: ToolSchema;
  result: ToolSchema;
  execute(args: TArgs, ctx: ToolContext): Promise<TResult>;
}

export type ToolSchema =
  | { type: "string"; enum?: readonly string[] }
  | { type: "number" }
  | { type: "boolean" }
  | { type: "null" }
  | { type: "unknown" }
  | { type: "array"; items: ToolSchema }
  | {
      type: "object";
      properties?: Record<string, ToolSchema>;
      required?: readonly string[];
      additionalProperties?: boolean | ToolSchema;
    }
  | { anyOf: readonly ToolSchema[] };

export interface ToolSchemaValidationResult {
  ok: boolean;
  errors: string[];
}

export type ToolExecutionErrorCode =
  | "TOOL_NOT_FOUND"
  | "INVALID_TOOL_ARGS"
  | "INVALID_TOOL_RESULT"
  | "TOOL_EXECUTION_FAILED";

export interface ToolExecutionError {
  code: ToolExecutionErrorCode;
  message: string;
  details: string[];
}

export interface ToolExecutionResult {
  toolName: string;
  callId: string;
  status: "ok" | "error";
  deduplicated: boolean;
  output?: unknown;
  error?: ToolExecutionError;
}

export const toolSchema = {
  string(options: { enum?: readonly string[] } = {}): ToolSchema {
    return options.enum ? { type: "string", enum: options.enum } : { type: "string" };
  },
  number(): ToolSchema {
    return { type: "number" };
  },
  boolean(): ToolSchema {
    return { type: "boolean" };
  },
  null(): ToolSchema {
    return { type: "null" };
  },
  unknown(): ToolSchema {
    return { type: "unknown" };
  },
  array(items: ToolSchema): ToolSchema {
    return { type: "array", items };
  },
  object(
    properties: Record<string, ToolSchema>,
    options: {
      required?: readonly string[];
      additionalProperties?: boolean | ToolSchema;
    } = {},
  ): ToolSchema {
    return {
      type: "object",
      properties,
      required: options.required ?? Object.keys(properties),
      additionalProperties: options.additionalProperties ?? false,
    };
  },
  anyOf(...schemas: ToolSchema[]): ToolSchema {
    return { anyOf: schemas };
  },
} as const;

export const ToolExecutionErrorSchema = toolSchema.object({
  code: toolSchema.string(),
  message: toolSchema.string(),
  details: toolSchema.array(toolSchema.string()),
});

export const ToolExecutionResultSchema = toolSchema.object(
  {
    toolName: toolSchema.string(),
    callId: toolSchema.string(),
    status: toolSchema.string({ enum: ["ok", "error"] }),
    deduplicated: toolSchema.boolean(),
    output: toolSchema.unknown(),
    error: ToolExecutionErrorSchema,
  },
  {
    required: ["toolName", "callId", "status", "deduplicated"],
    additionalProperties: false,
  },
);

export function createToolContext(args: {
  latestState: WorldStateV1;
  worldModel?: WorldModel;
}): ToolContext {
  const { latestState, worldModel } = args;
  const worldModelSnapshot = worldModel?.toJSON() ?? WorldModel.fromState(latestState).toJSON();

  return deepFreeze({
    latestState: cloneJson(latestState),
    worldModel: cloneJson(worldModelSnapshot),
  });
}

export function stableJsonStringify(value: unknown): string {
  return JSON.stringify(normalizeJsonValue(value));
}

export function validateToolSchema(schema: ToolSchema, value: unknown): ToolSchemaValidationResult {
  const errors: string[] = [];
  validateNode(schema, value, "$", errors);
  return {
    ok: errors.length === 0,
    errors,
  };
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function normalizeJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => normalizeJsonValue(item));
  }

  if (value && typeof value === "object") {
    const normalized: Record<string, unknown> = {};
    for (const [key, child] of Object.entries(value as Record<string, unknown>).sort(([left], [right]) =>
      left.localeCompare(right),
    )) {
      if (typeof child === "undefined") {
        continue;
      }

      normalized[key] = normalizeJsonValue(child);
    }
    return normalized;
  }

  return value;
}

function deepFreeze<T>(value: T): T {
  if (Array.isArray(value)) {
    for (const item of value) {
      deepFreeze(item);
    }
    return Object.freeze(value);
  }

  if (value && typeof value === "object") {
    for (const child of Object.values(value as Record<string, unknown>)) {
      deepFreeze(child);
    }
    return Object.freeze(value);
  }

  return value;
}

function validateNode(schema: ToolSchema, value: unknown, path: string, errors: string[]): void {
  if ("anyOf" in schema) {
    const matched = schema.anyOf.some((candidate) => validateToolSchema(candidate, value).ok);
    if (!matched) {
      errors.push(`${path}: value does not match any allowed schema`);
    }
    return;
  }

  if (schema.type === "unknown") {
    return;
  }

  if (schema.type === "string") {
    if (typeof value !== "string") {
      errors.push(`${path}: expected string`);
      return;
    }
    if (schema.enum && !schema.enum.includes(value)) {
      errors.push(`${path}: expected one of ${schema.enum.join(", ")}`);
    }
    return;
  }

  if (schema.type === "number") {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      errors.push(`${path}: expected finite number`);
    }
    return;
  }

  if (schema.type === "boolean") {
    if (typeof value !== "boolean") {
      errors.push(`${path}: expected boolean`);
    }
    return;
  }

  if (schema.type === "null") {
    if (value !== null) {
      errors.push(`${path}: expected null`);
    }
    return;
  }

  if (schema.type === "array") {
    if (!Array.isArray(value)) {
      errors.push(`${path}: expected array`);
      return;
    }
    for (let index = 0; index < value.length; index += 1) {
      validateNode(schema.items, value[index], `${path}[${index}]`, errors);
    }
    return;
  }

  if (!isRecord(value)) {
    errors.push(`${path}: expected object`);
    return;
  }

  const properties = schema.properties ?? {};
  const required = schema.required ?? Object.keys(properties);
  for (const key of required) {
    if (!(key in value)) {
      errors.push(`${path}.${key}: missing required property`);
    }
  }

  for (const [key, propertySchema] of Object.entries(properties)) {
    if (key in value) {
      validateNode(propertySchema, value[key], `${path}.${key}`, errors);
    }
  }

  for (const [key, propertyValue] of Object.entries(value)) {
    if (key in properties) {
      continue;
    }

    if (schema.additionalProperties === true) {
      continue;
    }

    if (schema.additionalProperties === false || typeof schema.additionalProperties === "undefined") {
      errors.push(`${path}.${key}: unexpected property`);
      continue;
    }

    validateNode(schema.additionalProperties, propertyValue, `${path}.${key}`, errors);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
