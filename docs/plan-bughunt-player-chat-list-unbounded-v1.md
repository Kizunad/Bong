# BugHunt: bong:player_chat Redis 队列无 LTRIM/TTL 上限，天道离线期间无界增长

## Bug 摘要

**严重度：medium**（skeptic 复核判定不变）。

`server/src/network/redis_bridge.rs::execute_outbound_command` 处理 `RedisIoCommand::ListPush` 时只发一条裸 `RPUSH key payload`（1845-1866 行），从来不配套任何 `LTRIM` / `MAXLEN` / `EXPIRE`。全仓 `grep -rn "LTRIM\|ltrim\|MAXLEN\|maxlen" server/src --include=*.rs` 对 `CH_PLAYER_CHAT`（`bong:player_chat`）及其他任何 Redis key 均零命中——这条队列从写入侧就没有任何长度或存活时间上限。消费侧 `agent/packages/tiandao/src/redis-ipc.ts::drainListAtomically` 只会 `LRANGE` + `LTRIM` 掉它成功读到的**已drain前缀**（`ltrim(key, maxItems, -1)` 只清走 `[0, maxItems)` 这一段），一旦生产速度超过 drain 速度，或天道 agent 进程干脆没在跑，超出 `maxItems`（默认 128，`CHAT_DRAIN_WINDOW`）的部分会一直原样堆在队尾，永不消失。

这是运维级资源泄漏而非玩法拒真元/拒宝物类 bug，但符合"IO 侧无界增长"这一类稳定性红旗：server 与 agent 是两个独立启动/重启的进程（`cargo run` vs `npm start`/`npm run start:mock`），任何一段"server 已起、agent 未连"的窗口叠加玩家持续聊天，都会让这个 Redis list 持续膨胀，长期可导致 Redis 内存压力甚至 OOM。

## 实际游玩体验影响

玩家侧感知不到直接异常（聊天照常发送、照常在游戏内聊天栏显示），bug 的代价落在运维侧：

- 天道 agent 崩溃 / 重启慢 / 版本升级重启窗口内，`bong:player_chat` 会持续吃掉玩家产生的每一条聊天（`chat_collector.rs::collect_player_chat` 仅做单人单 tick 3 条 + 单条 256 字符的限速限长，没有对总队列深度的任何限制），越忙的服（多人同时聊天）膨胀越快。
- 一旦 Redis 内存被这条无界队列占满，会连累其余共享同一 Redis 实例的功能（`bong:world_state`、`bong:agent_cmd` 等其它频道/key）一起遭殃，属于典型的"一个坏组件拖垮全局"故障模式。
- 天道 agent 重新上线后，`drainPlayerChat` 默认单次只吃 128 条最旧的——如果积压已经远超 128，agent 会看到严重滞后（几分钟甚至几十分钟前）的聊天，narration 响应相对于玩家当下语境显著过时，间接影响"天道感知世界"体验的实时性，且需要多轮才能追上积压（每轮只清 128 条最旧的，其余仍在原地）。

## 证据定位

- `server/src/network/redis_bridge.rs:597-608`（`prepare_outbound_command` 的 `RedisOutbound::PlayerChat` 分支）：只做 `validate_chat_message` + 序列化，产出 `RedisIoCommand::ListPush { key: CH_PLAYER_CHAT, payload }`，不带任何长度/存活期参数。
- `server/src/network/redis_bridge.rs:2432-2447`（`validate_chat_message`）：只校验 `chat.v == 1` 和 `chat.raw` 字符数 ≤ `CHAT_MESSAGE_MAX_LENGTH`（256），不涉及队列深度。
- `server/src/network/redis_bridge.rs:1845-1866`（`execute_outbound_command` 的 `RedisIoCommand::ListPush` 分支）：`redis::cmd("RPUSH").arg(key).arg(payload).query_async::<i64>(pub_conn)`，裸 RPUSH，无配套 LTRIM/EXPIRE。
- `server/src/network/redis_bridge.rs:315-330`（`RedisIoCommand` 枚举定义）：`ListPush { key, payload }` 只有这两个字段，没有长度上限字段；全仓仅 `PlayerChat` 一处产出该变体（`grep -n "RedisIoCommand::ListPush" server/src/network/redis_bridge.rs` 命中 324/604/1845/2882/3110，其中 2882/3110 是同一条既有测试 `pushes_chat_messages` 的断言），即这是一个单一用途、可以直接改造的通路。
- `server/src/network/chat_collector.rs:28-29`：`CHAT_MESSAGE_MAX_LENGTH = 256`、`MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK = 3` 只限单条长度和单人单 tick 速率，完全没有队列总深度概念。
- `server/src/schema/channels.rs:3`：`pub const CH_PLAYER_CHAT: &str = "bong:player_chat";`（key 的唯一真相源）。
- `agent/packages/tiandao/src/redis-ipc.ts:1001-1029`（`drainListAtomically`）：`pipeline = this.pub.multi().lrange(key, 0, endIndex).ltrim(key, trimStart, -1)`，其中 `endIndex = maxItems - 1`、`trimStart = maxItems`——只清走 `[0, maxItems)` 这一段被成功读出的前缀，`[maxItems, ∞)` 原样保留，队列可以在 `maxItems` 之外无限增长。
- `agent/packages/tiandao/src/redis-ipc.ts:106`：`DEFAULT_CHAT_DRAIN_WINDOW = 128`（`drainPlayerChat` 默认值）。
- `agent/packages/tiandao/src/runtime.ts:78`：`const CHAT_DRAIN_WINDOW = 128;`；`runtime.ts:1272-1275` 实际调用 `redis.drainPlayerChat({ maxItems: CHAT_DRAIN_WINDOW, logger })`——这是唯一的消费入口，且只在 agent 主循环存活时才会执行。
- 运维层面：仓库内未见任何 `maxmemory-policy` 之类的 Redis 侧兜底配置对该 key 生效（应用层是唯一的把关点）。

## 触发路径

1. server（Valence）正常启动并接受玩家连接，`chat_collector.rs::collect_player_chat` 对每条普通聊天做长度/单人单 tick 限速后，产出 `RedisOutbound::PlayerChat`。
2. `prepare_outbound_command` 转成 `RedisIoCommand::ListPush { key: CH_PLAYER_CHAT, payload }`，`execute_outbound_command` 对其执行裸 `RPUSH`——每条聊天都无条件追加进队列尾部。
3. 天道 agent 进程（独立 OS 进程）处于任意"未运行 / 未连上 / 重启中"的窗口——这是项目约定里完全正常的场景（crash、redeploy、先起 server 后起 agent 的常见 dev 顺序、agent 版本升级重启等），不需要任何异常操作。
4. 只要该窗口内还有玩家在正常聊天，`bong:player_chat` 就持续 RPUSH，没有任何东西在拉低它的长度。
5. agent 恢复后，`drainPlayerChat({ maxItems: 128 })` 每次只清走队头最旧的 128 条；如果积压量 ≫ 128，队列长度只是缓慢下降而非清零，且 agent 短期内消费到的都是严重滞后的旧消息。
6. 若窗口足够长（长时间下线、遗忘重启、生产环境静默故障等），队列长度可无限增长，直到 Redis 侧内存耗尽波及其它共享该实例的 key/频道。

## 反方审查记录

- 第一轮质疑：
  - 会不会 Redis 侧本身有 `maxmemory-policy` 之类的兜底？查 `docker-compose.test.yml` 的 redis service 配置：未设置 `maxmemory-policy`，不构成兜底。
  - 会不会消费侧的 `drainListAtomically` 其实是"整队列清空"而非"只清前缀"？逐行核对 `lrange(key, 0, endIndex)` + `ltrim(key, trimStart, -1)`：`trimStart = maxItems`，`LTRIM key maxItems -1` 的语义是"只保留 `[maxItems, -1]` 这一段"，也就是**扔掉刚读出来的 `[0, maxItems)` 前缀、保留 `[maxItems, ∞)` 尾部**——证实确实只清已drain的前缀，未读到的尾部原样保留，不构成整体上限。
  - 会不会生产侧本身已有总量限制、只是没叫 LTRIM 这个名字？查 `chat_collector.rs` 全部常量：`CHAT_MESSAGE_MAX_LENGTH`（单条字符数）、`MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK`（单人单 tick 条数）——均是"单条/单人"维度限制，没有任何"队列总深度"维度的限制。
  - 会不会是已知问题、已有 in-flight plan 覆盖？`grep -rln "player_chat\|PLAYER_CHAT" docs/plan-*.md docs/plans-skeleton/*.md docs/finished_plans/*.md` 命中 `plan-bughunt-r7-findings-v1.md`（#7chat：只是 `channels.ts` 里 BLPOP 注释文案与实际 LRANGE/LTRIM drain 实现不符的**文档纠错**，与队列无界增长完全是两个问题）与 `plan-bughunt-chat-signal-stale-window`（讨论聊天信号时间戳新鲜度/陈旧窗口判定，同样是另一个失效模式），均未覆盖"写入侧无 LTRIM/MAXLEN 上限"本身。
  - 初裁：倾向通过，属未被覆盖的真实运维缺口。
- 第二轮补证：
  - 补充确认 `RedisIoCommand::ListPush` 全仓仅 `PlayerChat` 一处产出（`grep -n "RedisIoCommand::ListPush"` 命中 324 定义、604 产出、1845 执行、2882/3110 既有测试断言），修复面单一、不会波及其它 Redis key。
  - 补充确认现有单测 `pushes_chat_messages`（redis_bridge.rs:2876-2892）只断言 `prepare_outbound_command` 产出的 `RedisIoCommand::ListPush{key, payload}` 内容，从未断言过队列深度或配套 LTRIM，佐证这个缺口此前完全没被测试撞到过。
  - 让步：这是"资源持续增长"型缺口，不是"单次触发即崩服/即吞真元"型 bug——触发条件需要"agent 离线窗口 + 持续聊天"叠加才会显现代价，严重度维持 medium 不上调不下调。
  - 终裁：通过。修复方向明确为"写入侧兜底"（RPUSH 配 LTRIM/MAXLEN），**不改动消费侧语义**（agent 仍按 `maxItems` 批量 drain 最旧的一批，行为对存活中的 agent 完全无感知）。

主循环复核：已亲读关键行确认。

## Skeleton Fix Plan

- [ ] 在 `server/src/network/redis_bridge.rs` 中为 `bong:player_chat` 定义一个显式、可配置的队列上限常量（如 `const PLAYER_CHAT_QUEUE_MAX_LEN: i64 = 4096;`，放在 `REDIS_IO_TIMEOUT` 附近同一批"运行时防护参数"里），数值需明显大于 agent 单次 drain 窗口 `CHAT_DRAIN_WINDOW`（128）留出安全边际，同时给出量级依据（如"约合 X 名玩家在 agent 离线 Y 分钟内的正常聊天速率上限"）写进代码注释。
- [ ] 让 `execute_outbound_command` 处理 `RedisIoCommand::ListPush` 时，在同一次 Redis round-trip 里追加 `LTRIM key -PLAYER_CHAT_QUEUE_MAX_LEN -1`（保留队尾最新的 N 条、丢弃更旧的），可用 `redis::pipe().rpush(key, payload).ltrim(key, -max_len, -1).query_async(pub_conn)` 一次流水线发出；不要求 MULTI/EXEC 事务级原子性（单一 writer 单连接下 RPUSH→LTRIM 严格顺序执行已经足够正确），但要在 pipeline 失败/超时路径上保留现有 `REDIS_IO_TIMEOUT` 语义（超时仍报错，不吞失败）。
- [ ] 显式声明并写进代码注释/plan 的丢弃策略：**丢最旧**（LTRIM 保留队尾最新 N 条），这是唯一允许的行为——不做"丢最新"或"整队列清空"。
- [ ] 加一条 `tracing::warn!` 分支：当本次 RPUSH 后的 `list_len`（即 execute 分支里已经拿到的返回值）超过 `PLAYER_CHAT_QUEUE_MAX_LEN` 时打日志，暴露"聊天队列正在被截断，说明 agent 侧长期没在消费"这一运维信号，避免这个上限本身变成又一个静默吞消息的黑洞。
- [ ] 不改动 `agent/packages/tiandao/src/redis-ipc.ts::drainListAtomically`、`runtime.ts` 的 `CHAT_DRAIN_WINDOW`/`drainPlayerChat` 调用点——agent 侧消费语义（一次最多 drain `maxItems` 条、`LRANGE`+`LTRIM` 清走已读前缀）保持不变；本次修复只加"写入侧兜底"，不引入消费侧的行为变化。
- [ ] 若 `RedisIoCommand::ListPush` 未来需要被其他 `RedisOutbound` 变体复用（当前全仓仅 `PlayerChat` 一处），评估是否要把 `max_len` 做成该枚举变体的字段（而非硬编码在 `execute_outbound_command` 内部）以便按 key 差异化配置；本 plan 范围内可先按"`ListPush` 恒配 `PLAYER_CHAT_QUEUE_MAX_LEN`"最小实现，不提前泛化。
- [ ] 复核 `docker-compose.test.yml`（以及生产部署配置，若仓库内有对应文件）是否需要补充 Redis 层 `maxmemory`/`maxmemory-policy` 作为纵深防御的第二层兜底；若不在本 plan 范围内，至少在 Finish Evidence 里写明"应用层已兜底，基础设施层兜底留作后续"。

## 验收测试计划

`server/` cargo test（`server/src/network/redis_bridge.rs` 内 `#[cfg(test)] mod tests`）：

- **happy path**：单条 `RedisOutbound::PlayerChat` 走 `prepare_outbound_command` 产出的命令结构，断言仍能被 `execute_outbound_command`（或其重构后的等价路径）识别为"RPUSH + 配套 LTRIM"，而不是回归成裸 RPUSH；沿用/扩展既有 `pushes_chat_messages` 测试断言 key 仍为 `CH_PLAYER_CHAT`、payload 内容不变。
- **边界（低于上限）**：模拟队列长度远小于 `PLAYER_CHAT_QUEUE_MAX_LEN` 时执行 RPUSH+LTRIM，断言 LTRIM 是 no-op（不丢任何已有元素）——即"未触发截断"分支不影响正常消息保留。
- **边界（恰好等于上限，off-by-one）**：队列长度恰为 `PLAYER_CHAT_QUEUE_MAX_LEN` 时再 push 一条，断言 push 后经 LTRIM 结果长度仍为 `PLAYER_CHAT_QUEUE_MAX_LEN`（丢掉且仅丢掉队头最旧的 1 条，不多丢不少丢）。
- **边界（远超上限）**：模拟"agent 长期离线、连续 push N ≫ `PLAYER_CHAT_QUEUE_MAX_LEN` 条"场景，断言队列长度全程被钳制在 `PLAYER_CHAT_QUEUE_MAX_LEN`，且保留的是最新的一批（丢最旧策略），不是保留最旧丢最新。
- **错误分支**：Redis 命令超时/连接失败时，断言现有 `Err(format!("failed to RPUSH {key}: {error}"))` / 超时分支的错误语义不因加了 LTRIM 而改变（仍然正确冒泡失败，不会因为流水线里多了一步而吞掉错误或误报成功）。
- **状态转换**：写入侧从"未触发截断"→"触发截断"→"截断后持续写入仍保持钳制"三段状态各有专属断言，配合上面的 `tracing::warn!` 分支，断言截断确实发生时该日志分支被命中（可用现有测试里对 tracing subscriber 的捕获方式，或至少断言触发条件的布尔判断函数本身）。
- **契约不变性**：新增一条测试锁定"`drainListAtomically`/`drainPlayerChat` 消费语义未变"——即本 plan 修复**不需要**在 agent 侧（`agent/packages/tiandao` vitest）新增或修改测试；若 skeleton 消费阶段发现确实需要触碰 agent 侧代码，必须先证明为什么"只改写入侧"不足以修复，再决定要不要动 consumer。

可选联调（非 gate，锦上添花）：临时把天道 agent 进程停掉，用真实 Redis 连续发送聊天到接近/超过设定上限，`redis-cli LLEN bong:player_chat` 观察长度被钳制在预期上限、agent 重新拉起后仍能正常 `drainPlayerChat` 而不报错。

## 风险

- 上限数值（`PLAYER_CHAT_QUEUE_MAX_LEN`）选得过小会导致"agent 短暂离线也丢历史聊天"影响天道对近期语境的感知；选得过大又起不到防护作用——需要在 fix 实现阶段结合真实运维经验（agent 典型重启耗时、单服典型聊天速率）敲定具体数值，plan 本身只锁定"必须有限且丢最旧"这一约束，不锁死具体数字。
- 若未来出现其它 Redis list 类 outbound（当前 `RedisIoCommand::ListPush` 仅 `PlayerChat` 一处使用），本修复若把上限硬编码在 `execute_outbound_command` 内部而非做成按 key 可配置，后来者复用 `ListPush` 时可能被这个专为聊天设计的上限意外套用；实现时需在代码注释里显式标注"当前上限专为 `CH_PLAYER_CHAT` 设计，新增 list 类 key 前先评估是否需要独立上限"。
- 修复点必须落在写入侧（`execute_outbound_command`），不能把兜底寄望于消费侧 `drainListAtomically` 加大 `maxItems`——那只是让"多久之后才会真正无界增长"的窗口变大，没有解决"agent 完全不在线时队列无限增长"的根本问题。
- 加 LTRIM 后如果 `PLAYER_CHAT_QUEUE_MAX_LEN` 设置不当导致正常运行下（agent 一直在线、drain 及时）也频繁触发截断日志，会制造噪音日志；需要确保正常运行路径下 `list_len` 长期远低于上限，触发日志只在真正异常的积压场景出现。
