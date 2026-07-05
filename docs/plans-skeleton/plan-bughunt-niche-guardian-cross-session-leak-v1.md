# plan-bughunt-niche-guardian-cross-session-leak-v1

> **骨架**（2026-07-05）。一句话主题：`NicheGuardianStore` 是纯 client 侧事件累积 store，但没有任何断线清理或清空协议；一旦收到 `niche_guardian_fatigue` / `niche_guardian_broken` / `niche_intrusion`，灵龛守护 HUD 会跨 session 挂着旧世界的状态继续显示，直到玩家重启客户端或刚好被新的灵龛事件覆盖。

## 复现路径

1. 在任意会触发灵龛守护事件的存档 / 服务器 A 上，制造一次 `niche_guardian_fatigue`、`niche_guardian_broken` 或 `niche_intrusion`。
2. 观察 client 已经把事件写进 `NicheGuardianStore`，HUD 右侧出现 `灵龛守护` 面板，内容来自 `guardianStatuses` / `intrusionAlerts`。
3. 直接断线，回到标题界面，随后进入另一张没有任何灵龛数据的新存档 / 服务器 B。
4. 不做任何新的灵龛交互，观察 HUD。

**实际结果**：
- `灵龛守护` 面板继续显示 A 局留下的 `guardianKind x charges` / `broken` / `龛侵 <intruder>` 文案。
- 因为这条链路没有 reset event，也没有 disconnect clear，旧面板会一直留到客户端进程重启，或恰好被 B 局新的灵龛事件覆盖。

**预期结果**：
- 断线 / 切服后应清空纯 client 临时态；B 局在没有收到任何 `niche_*` 事件前，不应凭空显示 A 局的灵龛守护信息。

## 根因链路

1. `SocialServerDataHandler` 对 `niche_intrusion` / `niche_guardian_fatigue` / `niche_guardian_broken` 的处理全是“收到事件就写本地 store”，没有任何“全量快照”或“inactive/reset”分支。
   - `client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:210-241`
2. `NicheIntrusionAlertHandler` 继续把事件写进 `NicheGuardianStore`：
   - `recordIntrusion()` 追加 `intrusionAlerts`
   - `recordGuardianFatigue()` 覆盖 `guardianStatuses`
   - `recordGuardianBroken()` 覆盖 `guardianStatuses` 且再补一条 intrusion
   - `client/src/main/java/com/bong/client/social/NicheIntrusionAlertHandler.java:38-79`
3. `NicheGuardianStore` 只有 `record*` 与 `resetForTests()`；没有 `clear()` / `clearOnDisconnect()`，也没有 TTL / 过期裁剪。
   - `client/src/main/java/com/bong/client/social/NicheGuardianStore.java:8-57`
4. HUD 侧是“只要 store 非空就渲染”，并且注释明确写了“面板即常驻显示直到状态被覆盖”。
   - `client/src/main/java/com/bong/client/social/NicheGuardianHudPlanner.java:14-17`
   - `client/src/main/java/com/bong/client/social/NicheGuardianHudPlanner.java:29-34`
5. 全局 disconnect 清理清单漏掉了 `NicheGuardianStore`。`BongNetworkHandler` 在 `ClientPlayConnectionEvents.DISCONNECT` 里清了大量 HUD / store，但没有这项。
   - `client/src/main/java/com/bong/client/BongNetworkHandler.java:131-170`
6. 全仓 grep 仅能找到 `resetForTests()`，找不到任何生产代码调用 `NicheGuardianStore.clear*`，因此旧状态一旦写入，就没有自然退出路径。

## 影响面

- **跨 session 串局**：玩家切服、重连、换存档后，HUD 继续展示上一局的灵龛守护剩余次数、破损状态、入侵者信息，误导当前决策。
- **UI consumer 误判**：`NicheGuardianHudPlanner` 会把“旧世界曾有守家载体 / 曾被谁入侵过”错当成当前世界事实，和实际服务器状态脱节。
- **恢复条件错误**：当前链路不是“临时闪一下”的 toast，而是常驻 HUD；没有新事件就不会自愈，严重度高于一次性提示串局。

## 修复建议

1. 给 `NicheGuardianStore` 增加生产态清空 API（`clear()` 或 `clearOnDisconnect()`），语义对齐已有 `SocialStateStore.clearOnDisconnect()` / `NpcInteractionLogStore.clearOnDisconnect()`。
2. 在 `BongNetworkHandler` 的 `ClientPlayConnectionEvents.DISCONNECT` 清单里补调它，和其他 client 临时 HUD store 一起清。
3. 可选加一条“进入新 session 时 HUD 应为空”的 pin 测试，防止未来再次漏接 disconnect hook。

## 验收抓手

- **最小回归**：A 局触发一次 `niche_guardian_broken` 或 `niche_intrusion` → 断线 → 进入 B 局且不触发任何灵龛事件 → `NicheGuardianHudPlanner.buildCommands()` 应返回空列表。
- **store 级 pin**：断线钩子执行后，`NicheGuardianStore.guardianStatuses().isEmpty()` 与 `intrusionAlerts().isEmpty()` 均为 true。
- **负向回归**：同一局内正常收到 `niche_guardian_fatigue` / `niche_guardian_broken` 时，HUD 仍然正常显示；修复只影响跨 session 清理，不改变事件消费。

## 反方裁决

### Round 1

**反方论点**：这可能不是 bug；灵龛守护面板设计上就是“常驻直到状态被覆盖”，跨重连保留也许是有意的“记事板”。

**驳回理由**：
- 这是纯 client 内存 store，不是持久化日志，也没有任何 account/world 维度隔离字段；如果真要跨局保留，至少需要显式 persistence 或 world/session key，而不是裸静态变量。
- 同类 client 临时态（`SocialStateStore`、`NpcInteractionLogStore`、`TiandaoPresenceStore` 等）都在断线时清空；当前实现更像漏接清理，而不是刻意做“跨世界记忆”。
- `NicheGuardianHudPlanner` 渲染的是“当前守家载体还有几次”“刚刚谁入侵了灵龛”，这是强上下文 HUD，不是历史档案。把 A 局实时状态投到 B 局，没有合理玩法解释。

### Round 2

**反方论点**：即使 disconnect 没清，服务器也许会在新局很快推一条空状态或新的 `niche_*` 事件把它覆盖，实际影响可能很短。

**驳回理由**：
- 这条协议根本没有“空快照 / reset / inactive”类型；`SocialServerDataHandler` 只处理 3 种增量事件，且全是 `record*` 写入，没有任何清空分支。
- `NicheGuardianStore` 本身也没有 TTL；若 B 局从未发生灵龛事件，旧面板会无限期残留。
- `NicheGuardianPanel.buildLines()` 在没有 guardian 状态时甚至会生成“无守家载体”，这意味着只要 `intrusionAlerts` 仍非空，面板就一定继续挂着，不存在被动自愈。

## 去重与退化处理

- 已避开已知题：toast cross-session、`false_skin_state` 残留、毒蛊 v2 HUD 串局、identity panel stale session、`zone_info` 同区不刷新、surface stash 标签缺口、realm gate 广播泄漏。
- 实地 grep `docs/plans-skeleton`、`docs/finished_plans`、`docs/plan-*`，未见现成的 `NicheGuardianStore` 跨 session / disconnect 清理题。
- 本会话未启用 subagent；两轮反方裁决改为本地人工对抗式复核。该退化处理需在后续 PR 正文如实记录。
