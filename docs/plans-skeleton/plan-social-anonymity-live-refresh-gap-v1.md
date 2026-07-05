# plan-social-anonymity-live-refresh-gap-v1（骨架）

> **骨架（草案）**。一句话主题：`SocialExposureEvent` 已把 server 权威 `Anonymity.exposed_to` 写进在线玩家社交状态并落盘，但 `social_anonymity` 只在 join 时下发一次，后续暴露只发 `social_exposure` 事件日志、不刷新匿名可见性快照；结果是 **server 侧已经判定“某 witness 识破了你”，client 世界内名牌却继续隐藏到重登为止**。影响是：聊天暴露、交易暴露、死亡暴露、天道点名暴露这几条核心匿名玩法都出现“规则已生效、在线观感没跟上”的断链。

> 立项动机：这是典型的 server runtime / gameplay bridge 缺口。`plan-social-v1` 已把“server 权威维护 `exposed_to`，client 据此显示/隐藏 name tag”定成匿名系统的主链，但当前 `origin/main` 只完成了初次 join 快照，漏了“暴露发生后的 live refresh”。该缺口位于多人在线主玩法链，且会让匿名/识破的即时反馈失真，值得先立 skeleton 收口证据、影响面、修复面与验收抓手，再由后续 fix PR 单独落地。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 暴露后 `social_anonymity` 不 live refresh，远端名牌/识别持续陈旧 | fix_pr | ⬜ |

## P0 — 暴露后 `social_anonymity` 不 live refresh，远端名牌/识别持续陈旧

- **现象**：`server/src/social/mod.rs` 的 `apply_social_exposures` 会把 `SocialExposureEvent.actor` 的 `Anonymity.exposed_to` 与 `ExposureLog` 正式写入在线组件/持久化，并给 actor+witnesses 下发 `ServerDataPayloadV1::SocialExposure`；但同文件里 `ServerDataPayloadV1::SocialAnonymity` 的发送点只有 `emit_anonymity_payloads_for_joined_clients`，触发条件是 `Added<Anonymity>`，也就是**仅 join 首次快照**。暴露发生后没有任何二次 `social_anonymity` 发送路径。
- **多人在线可达链路**：
  1. A、B 两名玩家在线进入同一 server，join 时各自收到一次 `social_anonymity` 全量快照，彼此默认匿名。
  2. A 触发聊天/交易/死亡/天道点名等暴露源，`apply_social_exposures` 将 B 写入 `A.anonymity.exposed_to`，并持久化到 `social_anonymity` 表。
  3. server 只给 A/B 发 `social_exposure` 事件日志，不给 B 重发“从 B 视角看 remotes”的匿名可见性快照。
  4. client 侧 `SocialServerDataHandler.handleExposure()` 仅 `recordExposure()` + HUD event；`SocialStateStore.shouldShowRemoteNameTag()` 读取的仍是旧的 `anonymity.remotesByUuid`。
  5. `client/src/main/java/com/bong/client/mixin/MixinEntityRenderer.java` 继续按旧快照隐藏 A 的 name tag；**B 必须等到重登/重连触发新的 `social_anonymity` 快照，才会看到 A 已暴露后的具名显示**。
- **为什么这是 bug，不是设计**：`docs/finished_plans/plan-social-v1.md` 的 Phase 0/1 已把匿名系统定为“`Anonymity` 由 server 权威维护 `exposed_to`，暴露事件触发时写入；server 下发 `AnonymityPayload` 给每个 client，client 据此显示/隐藏 name tag”。当前实现完成了“写入”和“首次快照”，却缺失“暴露后的 live refresh”，与设计口径不符；而且 `social_exposure` 已经只精准发给 actor+witnesses，说明 runtime 语义本来就是**即时在线反馈**，不是“等下次登录再看”的延迟型日志。
- **对实际游玩体验的影响**：
  - witness 已经在规则上识破了对方，但世界里仍看到“无名修士”，无法把刚刚的聊天/交易/死亡事件与眼前玩家实体即时对应起来；
  - 匿名博弈被错误延长，暴露者能在本次在线会话里继续吃到本不该存在的匿名收益，直到对方重登才补算；
  - server 落盘与 client 在线显示分叉，同一条暴露事件会出现“生效了，但没显示”的割裂，最容易误导联调与玩家判断。
- **根因链路**：
  - server 权威状态：`apply_social_exposures` 正式更新 `Anonymity.exposed_to`；
  - server→client bridge：只发送 `social_exposure` 日志，不发送新的 `social_anonymity` 可见性快照；
  - client 状态消费：`handleExposure()` 不修改 `SocialStateStore.anonymity`；
  - 视觉消费：`shouldShowRemoteNameTag()` / `MixinEntityRenderer` 只读 `anonymity` 快照，完全感知不到新的暴露关系。
- **建议修复范围 / 模块**：优先收口 `server/src/social/mod.rs`、`client/src/main/java/com/bong/client/network/SocialServerDataHandler.java`、`client/src/main/java/com/bong/client/social/SocialStateStore.java`。推荐主修方向是 **server 在 `apply_social_exposures` 后按参与者视角重发 `social_anonymity`**（至少 actor + witnesses；非 witness 不得泄漏），保持匿名可见性由 server 权威快照驱动；client 侧可选补一层 defensive delta apply，但不应把最终权威下沉成“只靠 event 猜状态”。
- **验收抓手**：至少补 5 组 pin。
  1. A 向 B 暴露后，**不重登**条件下 B 的 `shouldShowRemoteNameTag(A)` 立即翻为 `true`。
  2. 不在 witness 集合内的 C 仍保持看 A 匿名，防止 live refresh 误广播。
  3. 聊天/交易/死亡/天道点名四类 `SocialExposureEvent` 都走同一刷新路径，不能只修一条来源。
  4. client 断线重连仍能从 `social_anonymity` 快照恢复正确状态，live refresh 与 join snapshot 口径一致。
  5. 现有 `SocialServerDataHandlerTest` 要新增“暴露后匿名可见性翻转”的覆盖，避免继续只测事件日志、不测名牌可见性。

## 反方裁决摘要

> **退化说明**：本会话没有可用 subagent/委派工具，无法按理想流程再开两轮外部子代理对抗；这里如实退化为主代理本地两轮反方裁决，并在 PR 正文同步记录这一点。

1. **Round 1 反方论点**：也许 `social_exposure` 本来就只是 HUD 日志，“被识破后的具名显示”是故意设计成下次登录才刷新。  
   **驳回理由**：`plan-social-v1` 明文要求 server 权威维护 `exposed_to`，并下发 `AnonymityPayload` 让 client 直接显示/隐藏 name tag；`apply_social_exposures` 也已经把日志精准推给 actor+witnesses，而不是全服归档型广播，语义明显是在线即时反馈，不是离线结算。
2. **Round 2 反方论点**：也许 client 收到 `social_exposure` 后已经能自己推导出名牌应当显示，所以不需要 server 重发 `social_anonymity`。  
   **驳回理由**：`SocialServerDataHandler.handleExposure()` 只 `recordExposure()`；`SocialStateStore.shouldShowRemoteNameTag()` 只读取 `anonymity.remotesByUuid`；`MixinEntityRenderer` 又只看这个布尔值决定是否 cancel 名牌渲染。当前代码链不存在任何“从 exposure 事件回写 anonymity cache”的路径，因此 live nametag 绝不会翻转。
3. **人工复核结论**：这是完整的三段断链而非单点疏漏：server 权威状态已变、bridge 未刷新、client 视觉继续读旧快照。两轮反方都没能提供代码级反证，候选保留。

## 开放问题

1. `social_anonymity` live refresh 应该做成**全量 per-viewer 重发**，还是补一个更小的 delta payload（例如仅刷新 actor 这一条 remote identity）？推荐先走全量 per-viewer，复用既有 `build_remote_identity_payloads`，避免再造第二套匿名可见性协议。
2. `social_exposure` client 侧是否仍要顺手做 defensive cache patch？如果保留，也应把它定位成“抗丢包/抗时序抖动”的辅助手段，而不是取代 server 权威快照。

## 审计来源

bug-hunt 定点轮（fresh worktree，主题：server runtime / gameplay bridge，优先查事件发射消费链、多人目标过滤、状态同步、server 已生效但 client/在线体验没跟上的断链）。本候选经本地双轮反方裁决 + 人工代码复核后保留。当前结论是 **report-only**：先提交 skeleton plan，把复现链、根因链、影响面、修复建议与验收抓手讲清，再由后续 fix PR 单独落地 live anonymity refresh。
