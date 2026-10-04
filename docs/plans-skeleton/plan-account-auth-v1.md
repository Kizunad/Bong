# plan-account-auth-v1 — 人与 agent 共用的账号认证与无头登录契约

> 一句话主题：把当前可冒充 `Username` 的 offline 接入，收口为人和 agent 共用的一套账号认证；Java 客户端本地没有登录记录时显示登录界面，无头客户端走同一挑战—应答契约，同时不向 offline transport 放入可重放 bearer 凭证。
>
> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md` §9.5（RF-45，2026-09-30 已决议）。本文件是骨架，不宣称认证已接入生产。

**状态**：骨架（skeleton）

**执行**：—

**PR**：—

## 接入面

- **进料**：现有 Valence 连接身份（`server/src/main.rs:126-130` 的 `NetworkSettings.connection_mode`、`Username`、`Lifecycle.character_id`）、账号登录输入、服务端生成的一次性 challenge，以及 Java/headless 客户端保存的本地登录记录（若有）。
- **出料**：统一的 authenticated account principal、绑定本次连接的 auth session、可供后续 R4/R6/C2S handler 使用的服务端授权上下文；认证成功或失败必须有机器可读结果，不能只靠聊天文案。认证层不直接修改境界、经脉、功法、身体部位或经济状态。
- **共享类型 / event / schema**：复用 `Username`、`Lifecycle.character_id`、`bong:server_data` 与现有连接 session 生命周期；新建的 `AuthChallengeV1`、`AuthResponseV1`、`AuthResultV1`（名称在 P0 冻结）必须成为唯一认证形状，禁止 client、bot、agent 各自发明凭证格式。不能把 `offline_uuid` 当认证证明。
- **跨仓库契约**：server 负责 challenge、验证、principal 绑定和撤销；`agent/packages/schema/src/channels.ts:5-16,30-40` 现有 Redis channel 继续只承载世界/聊天/叙事/经济等业务，不把 bearer secret 放进 Redis 业务 payload；client 由 `BongClient.onInitializeClient()`（`client/src/main/java/com/bong/client/BongClient.java:80-115`）注册登录 UI 与连接桥；headless 复用 `scripts/bot/mc_protocol.py::login`（`253-275`）之后的同一认证契约。
- **worldview 锚点**：`worldview.md §十一 L930-L961` 的匿名与多身份语义要求认证主体和对外显示身份分开；认证不得把账号主体直接广播为玩家可见姓名。`worldview.md §九 L945-L961` 的多身份/信誉记录继续属于 gameplay identity，不由本 plan 另造一套账号身份。
- **qi_physics 锚点**：认证不产生、转移或释放真元，不定义物理常数；认证失败、重试、撤销都不得触碰 `qi_current`、zone 灵气或 `WorldQiAccount`。

## 现状证据与边界

- 服务端现在显式使用 `ConnectionMode::Offline`（`server/src/main.rs:126-130`）；握手中的用户名可以被客户端自报，不能作为账号认证。
- Bot 的 `login` 只发送 MC 763 Handshake + offline `LoginStart`，遇到 `Encryption Request` 直接报错（`scripts/bot/mc_protocol.py:253-275`）；`offline_uuid` 由用户名 SHA-256 派生（`:297-305`），只能作为离线连接识别，不是秘密。
- Java 客户端目前注册网络 receiver 和连接 session 生命周期（`client/src/main/java/com/bong/client/BongNetworkHandler.java:111-193,336-364`），`BongClient` 只做 bootstrap（`:80-115`），未发现账号认证或本地登录记录 UI。
- `scripts/bot/run_scenarios.py::ScenarioEnv.new_bot`（`:29-43`）按用户名建 Bot，`lookup_character_id`（`:45-83`）从 SQLite 反查角色；这是测试 harness 的现状，不是未来认证授权。
- `docs/plan-client-login-ux-v1.md` 处理资源包 manifest、下载屏幕和资源包降级，不拥有账号认证；本 plan 不重复它，也不把资源包成功当作已登录。

## 防孤岛调研记录（2026-09-30）

- 已读 `docs/finished_plans/plan-client.md`、`plan-server.md`、`plan-agent.md`、`plan-agent-v2.md`、`plan-ipc-schema-v1.md`、`plan-social-renown-identity-bridge-v1.md`、`plan-bot-e2e-timing-flaky-v1.md`，并核对 active `plan-client-login-ux-v1.md`、R4、R6 与 V；它们分别覆盖连接体验、offline 身份投影、Redis/schema、社交 identity、Bot 稳定性或 C2S/S2C machinery，没有统一账号认证 plan。
- `plan-client-login-ux-v1` 的登录措辞只指资源包连接阶段，现状代码也只有 `BongNetworkHandler` session lifecycle；因此本 skeleton 新建认证 owner，不重名、不把资源包 UX 改写成账号系统。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 认证现状盘点、威胁模型、统一 principal 与 wire 形状冻结 | ⬜ |
| P1 | 账号/凭证生命周期：签发、轮换、撤销、过期与持久化边界 | ⬜ |
| P2 | server challenge—response 验证与连接/session 绑定 | ⬜ |
| P3 | Java 客户端登录 UI、本地无记录提示与安全本地记录 | ⬜ |
| P4 | headless/agent 登录适配、协议版本纪律与拒绝反馈 | ⬜ |
| P5 | 离线身份迁移、跨端契约 pin、bot e2e 与运营回滚证据 | ⬜ |

## P0 — 统一认证契约与现状基线 ⬜

- 逐项冻结 `AccountPrincipal`、`AuthChallengeV1`、`AuthResponseV1`、`AuthResultV1` 的字段、版本、nonce 生命周期、错误类别和连接绑定规则；最终名称必须在 server、client、agent/schema、bot 四方一致。
- 记录当前 offline 握手的可冒充边界：`ConnectionMode::Offline`、`mc_protocol.login`、`offline_uuid`、`Lifecycle.character_id`；明确 `Username`、角色 ID、账号 principal、session token 不能互相替代。
- 定义威胁模型：重放旧 response、跨账号/跨连接转用 response、nonce 复用、并发登录、断线重连、时钟偏差、未知版本、服务端重启、撤销后旧连接继续发请求；所有失败都 fail-closed，不把秘密写入日志、聊天、`bong:server_data` 业务快照或 Redis 业务 channel。
- 与 `docs/plan-refactor-c2s-gate-v1.md` 的 authenticated owner proof、`docs/plan-refactor-wire-s2c-v1.md` 的 generated transport machinery 对拍；认证计划只定义身份/证明契约，不越权实现 R4 gate 或 R6 全量生成。
- **契约测试**：正样本、缺字段、未知字段、过期 nonce、重复 response、跨连接 response、版本不支持、认证失败后的零 gameplay mutation；不以源码字符串 grep 代替行为断言。

## P1 — 账号与凭证生命周期 ⬜

- 设计一套同时服务人和 agent 的账号记录：账号 principal、可绑定的角色/`Lifecycle.character_id`、设备/客户端会话、签发时间、过期时间、撤销版本和审计事件；一个 agent 不得通过“特殊账号类型”绕过普通授权。
- 采用不可重放的 challenge—response 或等价的密码学证明。offline transport 只承载握手兼容字段，不承载可单独重放的 durable bearer；任何短期 response 也必须绑定 nonce、连接、账号和服务端 epoch。
- 明确签发、轮换、撤销、过期、并发设备与异常恢复语义。撤销/轮换只影响认证状态，不隐式改角色的修为、库存、信誉或经济余额。
- 持久化只保存必要的 verifier/公钥/版本元数据，不保存可直接登录的明文秘密；迁移失败必须阻止进入需要认证的 gameplay 路径，并提供可诊断的机器可读 reason。
- **契约测试**：成功签发→使用→轮换→旧证明失效；撤销后新请求拒绝；同一 response 在第二连接重放拒绝；服务端重启后 nonce/epoch 不接受旧证明；账号 principal 与角色 ID 不匹配时拒绝且状态零变化。

## P2 — server 验证与连接绑定 ⬜

- 在现有连接初始化与业务请求进入点之间增加统一 auth session seam；`ClientPlayConnectionEvents` 的 Java 生命周期（`BongNetworkHandler.java:162-193`）只负责 session 边界，不能自行决定服务端授权。
- 认证成功后把 principal、连接代次、challenge epoch 和撤销版本绑定到服务端连接；后续 R4 `RequestGate`、R6 `request_rejected` 和 owner proof 只消费这份中立上下文。
- 未认证连接只能收到认证所需的最小反馈；未知/坏包、乱序、重复、过大、错误版本都安全拒绝，不踢服、不 panic，也不执行 gameplay handler。
- 认证结果通过专用、可版本化的机器可读契约返回；不得把密码学细节、账户存在性或内部拒因暴露成 oracle。玩家可读提示由 client 本地模板决定。
- **契约测试**：非认证请求零 `ClientRequest` 副作用；坏包保持连接且下一合法认证可成功；认证通过后同一连接才可进入受保护 C2S；跨连接/跨角色/旧代次请求 fail-closed。

## P3 — Java 客户端登录 UI 与本地记录 ⬜

- 在 `BongClient` bootstrap 注册连接前登录屏幕与状态桥；复用现有 `ClientConnectionStatusStore`/`BongNetworkHandler` 的 session 边界，不在各业务 Screen 里各自保存登录状态。
- 本地没有登录记录时必须弹出登录 UI；有记录时先显示待验证状态，服务端要求重新证明、记录过期或撤销时回到登录 UI。UI 只呈现成功、失败、重试等待和服务不可用等安全折叠类别。
- 本地记录采用 P0/P1 冻结的安全存储策略；记录不得包含可被复制到 offline transport 的 durable bearer，不得写入日志、崩溃报告或 `bong:server_data`。
- 登录成功后再进入 gameplay；断线/切服只清本次 auth session 的内存态，不能把旧连接 principal 带给下一条物理连接。资源包连接体验仍由 `plan-client-login-ux-v1` 负责。
- **契约测试**：无记录启动显示登录 UI；成功记录可在下一次启动触发重新证明；撤销/过期回到登录 UI；断线重连不复用旧连接 nonce；登录失败不改任何 gameplay store。

## P4 — headless 与 agent 共用适配 ⬜

- 为 `scripts/bot/` 提供与 Java 客户端同一套认证消息和状态机；`mc_protocol.py::login` 保留 MC 763 framing 责任，认证层不得另造 bot-only secret 或 username 旁路。
- agent 账号通过同一账号 principal/凭证生命周期登录；agent runtime 只消费机器可读认证结果，不把 token 写进 `bong:player_chat`、`bong:agent_command`、`bong:agent_narrate` 或世界状态快照。
- 版本纪律：认证 wire、challenge 算法、错误枚举、nonce/epoch 语义变更必须同步 Java client、headless bot、server decoder、TypeBox/proto samples；不允许只升级测试脚本。
- **Bot 契约场景**：挑战成功后能完成一个受保护的只读请求；重复/延迟/跨连接 response 被拒绝且连接保持；未认证坏包不踢人、不 panic；授权账号与普通账号的业务权限差异由 R4 gate 验证，不在 bot 里绕过。

## P5 — 迁移、运营与全链验证 ⬜

- 为已有 `offline:<username>` 角色设计一次性迁移与回滚：旧角色数据不因认证缺失丢失；同名但未证明原账号的连接不得直接取得旧角色的 owner authority。
- 贯通 server → wire → Java/headless → agent 的正反 sample、版本 pin、freshness 和日志脱敏检查；把认证结果与 R4/R6 的 request correlation 对拍，但不重复定义 `request_rejected`。
- 增加 bot e2e：首次登录、已保存记录、过期/撤销、重放、断线重连、服务端重启、坏包宽容和认证成功后真实 gameplay request；CI 不把 Java UI 渲染当作 headless 证据。
- 发布/回滚手册必须能在不删除角色或经济数据的情况下停用新认证、冻结新连接并恢复已有会话；所有操作留下不含秘密的审计证据。

## 验收与不做

- 验收必须证明人和 agent 用同一账号认证契约、离线握手不含可重放 bearer、挑战 response 一次性且绑定连接/epoch、客户端无本地记录会显示登录 UI、坏包不踢人或 panic、撤销/轮换可观察且 gameplay 零误变更。
- 本 plan 不改 worldview、经济、修炼、经脉、功法/各流派招式、身体部位，不把匿名显示改成实名显示，不替 R4/R6 首次定义其 owner phase，也不把 `plan-client-login-ux-v1` 的资源包流程改名为账号认证。

## §8 开放问题（升 active / P0 决策门前收口）

1. 账号记录的 canonical store 是现有 SQLite persistence slice 还是独立认证服务；需要和 R3 的 Slice owner 对拍后再定。
2. challenge—response 的具体密码学方案、公钥/密码 verifier 形式、服务端 epoch 与密钥轮换周期；必须证明不可重放，但本骨架不预先拍算法或数值。
3. Java 客户端本地记录使用系统凭据存储、加密文件还是仅保存不可登录的账号提示；需要明确平台不可用时的降级行为。
4. 旧 `offline:<username>` 角色的账号认领与人工恢复流程；同名不能作为证明，迁移失败不能静默合并两份角色。
5. 认证 wire 归 R6 哪个 phase、与 R4 gate 的 production cutover 是否同一 merge unit；在对应 owner amendment 和总纲 Wave 表更新前只允许 contract-first。

## §10 实施工作流

### §10.1 多 PR 顺序

1. PR-1：P0 契约、现状审计、威胁模型和跨轨 owner amendment。
2. PR-2：P1/P2 server 认证原语与连接绑定，保持旧 offline 路径可回滚。
3. PR-3：P3 Java 登录 UI、本地记录和 session 清理。
4. PR-4：P4 headless/agent 适配与 bot 契约场景。
5. PR-5：P5 迁移、运营回滚和全链 e2e；全部完成后才归档本 plan。

### §10.2 每 PR 验证

- 按实际栈运行 server/client/agent/schema/bot 门禁；认证 wire 变更必须同时跑 TypeBox/proto/sample freshness 和协议级 bot 场景。
- 任何新提交都要从最新 `origin/main` fetch+merge 后重新验证受影响栈；认证失败、坏包和重放路径必须保留可诊断日志但不泄漏秘密。

### §10.3 review 与归档

- 每个 PR 用独立实现上下文，中文 commit 带 `Model:` trailer，PR body 写清 docs/code 范围与可读性自查；按当前 HEAD 对拍 Kody review。
- 只有所有阶段 `✅ YYYY-MM-DD`、Finish Evidence 完整、迁移回滚和 bot e2e 证据齐全后，才把 active plan 迁入 `docs/finished_plans/`。

### §10.4 PR 实施上下文

- 每个 PR 使用独立实施 subagent；prompt 必须带本 PR 的文件/阶段范围、本文威胁模型和验收要求，并要求先核对最新生产代码，不得依照旧 plan 名称猜 API。
- 实施 subagent 使用 `subagent_type: "claude"`、`model: "opus"`，prompt 末尾包含 `ultrathink`；它负责实现、对应栈门禁、push 和开 PR，不等待 review，也不 merge。
- 返工由新的独立 subagent 负责，继承当前 PR 分支和上一轮证据；不得重复 promotion、追加重复 Finish Evidence 或归档。

### §10.5 Kody review 等待

- PR 创建和每次 push 后按 `docs/CLAUDE.md §6.5` 等待 Kody；当前 HEAD 以 `gh pr view --json headRefOid` 为准，行内意见用 `original_commit_id` 对拍并通过 `gh api .../comments --paginate` 读取完整列表。
- Kody 反馈缺失时遵守 §6.5 的等待轮数；当前 HEAD 有成立意见必须先处理，再等待当前 HEAD 的明确 clean。若意见要求新的账号/密码学/迁移决议，停止并交人工拍板。

### §10.6 单次 consume-plan 到 merge

- 一次 `/consume-plan` 只推进当前最前一个未完成 PR；按依赖串行完成后续阶段。每个 PR 经门禁、最新主线复验、Kody 和 CI 收敛后，orchestrator 在既有授权边界内 squash merge；无 merge 授权则停在可合并状态。
- 用户无需手工拆分或补写阶段证据；所有阶段完成、Finish Evidence 齐全后，由最后阶段 PR 归档本 plan。

### §10.7 单次消费边界

- `/consume-plan` 每次只推进最前一个未完成阶段；认证算法、凭证存储或跨轨 cutover 尚未拍板时停在 P0/P1，写明缺口交人工，不在实现 agent 内自行决定。

## Finish Evidence

> 迁入 `docs/finished_plans/` 前填写：统一认证 source/wire/server/client/headless 路径、关键 commit 与日期、各栈门禁和 bot 场景、重放/撤销/迁移证据、跨轨 owner amendment 及遗留问题。
