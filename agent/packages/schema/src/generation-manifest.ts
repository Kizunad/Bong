/**
 * R6 P1 的 schema 生成链登记表。
 *
 * 这里记录每一段生成链的 owner、状态和产物边界；它不是业务 schema 的
 * 第二份 source of truth，也不会把 declared/unwired 的 transport 宣称为已上线。
 */

/** 当前生成器与每份 JSON Schema 共同携带的版本 pin。 */
export const SCHEMA_GENERATION_VERSION = "r6-p1-v1" as const;

/** 生成 manifest 自身的格式版本。修改 manifest 结构时递增。 */
export const SCHEMA_GENERATION_MANIFEST_VERSION = 1 as const;

/** 生成 manifest 在 `@bong/schema` 包根目录中的文件名。 */
export const SCHEMA_GENERATION_MANIFEST_FILE_NAME = "generation-manifest.json" as const;

/** 生成链阶段的状态只能表达声明、未接线或仅测试用途。 */
export const SCHEMA_GENERATION_STAGE_STATUSES = [
  "declared",
  "unwired",
  "test-only",
] as const;

export type SchemaGenerationStageStatus =
  (typeof SCHEMA_GENERATION_STAGE_STATUSES)[number];

/** 一段生成链的静态 owner 与接线边界。 */
export interface SchemaGenerationStage {
  readonly id: string;
  readonly status: SchemaGenerationStageStatus;
  readonly owner: string;
  readonly source: readonly string[];
  readonly output: readonly string[];
  readonly production_reachable: false;
  readonly note: string;
}

/**
 * R6 P1 只登记契约边界。
 *
 * 后续阶段可以沿用这些 id 追加真实 mirror/converter/transport pin，但必须
 * 显式改变状态并在同一 atomic activation 中接入 producer；本表不会自动接线。
 */
export const SCHEMA_GENERATION_STAGES = [
  {
    id: "typebox-to-json-schema",
    status: "declared",
    owner: "R6 P1",
    source: ["src/*.ts", "src/schema-registry.ts", "src/generate.ts"],
    output: ["generated/*.json"],
    production_reachable: false,
    note: "TypeBox registry 到 committed JSON Schema；由单入口 generate.ts 生成。",
  },
  {
    id: "json-schema-to-dist",
    status: "unwired",
    owner: "R6 P1 / 后续 schema 发布阶段",
    source: ["generated/*.json"],
    output: ["dist/*.js", "dist/*.d.ts"],
    production_reachable: false,
    note: "dist 是包构建产物，当前不作为 server/client transport 的生产来源。",
  },
  {
    id: "proto-generated-mirrors",
    status: "unwired",
    owner: "R6 P3",
    source: ["proto/bong/*.proto"],
    output: ["server/src/schema/proto_gen.rs", "client/build/generated"],
    production_reachable: false,
    note: "protobuf mirror 与生成代码仍由现有构建链维护，P1 不切换 producer。",
  },
  {
    id: "rust-converter",
    status: "unwired",
    owner: "R6 P3/P4",
    source: ["server/src/schema/proto_convert.rs"],
    output: ["server/src/schema/*_convert.rs"],
    production_reachable: false,
    note: "Rust conversion pin 留给 mirror/transport activation，不在 P1 宣称完成。",
  },
  {
    id: "java-bridge-router",
    status: "unwired",
    owner: "R6 P2/P3",
    source: [
      "client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java",
      "client/src/main/java/com/bong/client/network/ServerDataRouter.java",
    ],
    output: ["client/src/main/java/com/bong/client/network/*"],
    production_reachable: false,
    note: "Java bridge/router 仍沿用现有接收路径，P1 只冻结 generation 边界。",
  },
  {
    id: "craft-restore-guard-contract",
    status: "test-only",
    owner: "R6 P1（producer 由 R1、persistence 由 R3、consumer 由 R2）",
    source: ["A-CS A-01..A-08 frozen artifacts", "docs/plan-refactor-wire-s2c-v1.md"],
    output: ["CraftRestoreGuard { owner_key, session_key, generation, phase_revision, restore_token }"],
    production_reachable: false,
    note: "只登记控制帧的 owner/字段边界；真实 frame→Restore→Store cutover 由 M-10 放行。",
  },
] as const satisfies readonly SchemaGenerationStage[];
