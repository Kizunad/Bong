# plan-bughunt-tiandao-schema-dist-start-v1

## 一句话

`@bong/tiandao` 的 `start` / `start:mock` / `test` 直接运行源码并从 `@bong/schema` 导入运行时 validator，但 `@bong/schema` 的 package exports 只指向 `dist/`，干净 worktree 中 `agent/packages/schema/dist` 不存在，导致按子包命令启动 Tiandao 时会在模块解析阶段断链。

## 实际游玩体验影响

这不是某条 Redis payload 被 Tiandao 误判的单点问题，而是 agent 进程启动前置断链：运维或开发者按仓库矩阵在 `agent/packages/tiandao` 跑 `npm start` / `npm run start:mock` / `npm test` 时，Tiandao 需要解析 `@bong/schema` 的运行时导出；如果没有先手动构建 schema，Node 会按 `@bong/schema` 的 `exports.import` 找 `dist/index.js`，而干净 worktree 没有该产物。结果是天道进程起不来，所有依赖 Tiandao 的叙事、世界调度、UI-as-data 响应、Redis 事件消费都不可用。

## 证据

- `agent/packages/schema/package.json` 的 `main` / `types` / `exports["."].import` 都指向 `dist/index.js` / `dist/index.d.ts`。
- `agent/packages/tiandao/package.json` 的 `start` / `start:mock` 直接执行 `tsx src/main.ts`，`test` 只做 `tsc -p tsconfig.test.json --noEmit && vitest run`，没有前置 `npm run build -w @bong/schema`。
- `agent/packages/tiandao/src/redis-ipc.ts` 等运行时代码从 `@bong/schema` 导入 validator 和 channel 常量，这是运行时 import，不是纯 type import。
- 当前 worktree 下 `agent/packages/schema/dist` 不存在，`git ls-files agent/packages/schema/dist` 也为空。
- 本轮尝试 `cd agent && npm run start:mock -w @bong/tiandao` 在依赖未安装处先失败为 `tsx: not found`；因此本 plan 不把该命令输出当作模块解析复现，只记录 package exports 与缺失 dist 的静态断链。装好依赖后会进入 `@bong/schema` exports 指向缺失 dist 的风险路径。

## 去重

- 不重复 #1061 `agent schema 生成物覆盖假绿`：#1061 关注 TypeBox validator 已存在但 `SCHEMA_REGISTRY` / generated JSON schema freshness gate 漏覆盖，属于契约审计假绿；本问题关注 workspace 包的 ESM exports 指向未生成的 `dist/`，导致 Tiandao 子包脚本在干净环境下启动前模块解析断链。
- 不重复 #1054 NPC combat/relic schema parity、#1075 anticheat Tiandao runtime 无消费、#1081 niche guardian Redis dispatch schema 误解析。
- 不采用本轮被反方提出但已重复的候选：`AlchemyInsightV1.ts` 缺 `ts` 已由 #995 覆盖；`tsy_enter/tsy_exit` 漏消费已由 #1011 覆盖；`bone_coin_tick` runtime 已被历史 #803 覆盖；`territory_narration_request` server 明确有 `PendingGameplayNarrations.push_zone` 兜底且注释标为 future agent runtime。

## 修复方向

- 方案 A：让 `@bong/tiandao` 的 `prestart` / `pretest` / `predev` 或 workspace 根脚本保证先构建 `@bong/schema`。
- 方案 B：调整 workspace 开发态 exports / tsconfig path，使 Tiandao 源码运行时解析到 `@bong/schema/src`，发布态仍用 `dist`。
- 无论选哪条，都需要 pin 一个干净 worktree 场景：安装依赖后，不手动运行 schema build，直接跑 `npm run start:mock -w @bong/tiandao`，要么自动构建，要么明确解析到源码。

## 验收

- `cd agent && npm run start:mock -w @bong/tiandao` 在干净依赖安装后不因 `@bong/schema/dist` 缺失崩溃。
- `cd agent && npm test -w @bong/tiandao` 不依赖人工预先执行 `npm run build -w @bong/schema`。
- 若修改 schema src，仍保留 AGENTS 要求的 `cd agent && npm run build -w @bong/schema` 作为正式构建产物验证。

## 对抗结论

两轮 subagent 已完成。最终保留本候选：它属于 dist 构建产物引用风险，证据来自 package exports、tiandao 运行时 import 与干净 worktree 缺失 dist；与 #1061 的 generated schema coverage 假绿不是同一层问题。反方提出的 AlchemyInsight、TSY enter/exit、BoneCoinTick、territory narration 均因重复或 server 兜底降级而剔除。
