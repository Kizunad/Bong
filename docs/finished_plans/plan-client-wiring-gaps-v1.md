# Bong · plan-client-wiring-gaps-v1 · 骨架

**补全 server→client 数据管线断点**——三条 server 已发但 client 未接的 payload 管线。采集无进度反馈 + 交易推送到不了 client。纯 client 改动，server/agent 不动。

**交叉引用**：
- `plan-mineral-v1`（finished）：`mining_progress` payload + `send_mining_progress_to_client()`
- `plan-social-v1`（finished）：`trade_offer` payload + `SocialServerDataHandler.handleTradeOffer()`
- `plan-lumber-v1`（finished，如有）：`lumber_progress` payload

**前置依赖**：
- `ServerDataRouter` ✅（`client/.../network/ServerDataRouter.java`，104 个 handler 注册）
- `SocialServerDataHandler.handleTradeOffer()` ✅（`:244`——已实现，只差注册）
- `GatheringProgressHud` ✅（`client/.../hud/GatheringProgressHud.java`——渲染组件已有，缺数据源）
- `ServerDataPayloadV1::MiningProgress` ✅（`server/src/schema/server_data.rs:306`）
- `ServerDataPayloadV1::LumberProgress` ✅（`server/src/schema/server_data.rs:313`）
- `send_mining_progress_to_client()` ✅（`server/src/mineral/break_handler.rs:447`——server 已在发）

---

## 接入面 Checklist

- **进料**：server 已发的 `mining_progress` / `lumber_progress` / `trade_offer` payload
- **出料**：`GatheringProgressHud` 进度渲染 + 交易弹窗/通知
- **共享类型**：无新增
- **跨仓库契约**：仅 client（server 已在正确发送）
- **worldview 锚点**：无
- **qi_physics 锚点**：无

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | trade_offer 注册 + mining/lumber handler + store + HUD 接入 | ⬜ |
| P1 | 饱和测试 | ⬜ |

---

## P0 — 补全三条管线

### A. trade_offer 注册（1 行修复）

`SocialServerDataHandler` 内部已实现 `handleTradeOffer()`（`:244`），case 分发也有（`:35`），但 `ServerDataRouter.createDefault()` 漏注册。

**修复**：`ServerDataRouter.java` 在 social handler 注册块（`:203-211`）追加：
```java
handlers.put("trade_offer", socialServerDataHandler);
```

### B. mining_progress handler

新建 `MiningProgressHandler.java`（`client/.../network/`）：
- 解析 payload：`session_id`, `mineral_id`, `progress` (0.0-1.0), `display_name`, `remaining_units`
- 写入 `GatheringProgressStore`（新建 store 或复用已有 state）
- `GatheringProgressHud.buildCommands()` 已能渲染进度条——对接 store 即可

**注册**：`ServerDataRouter.java` 追加：
```java
handlers.put("mining_progress", new MiningProgressHandler());
```

### C. lumber_progress handler

同 B 模式，新建 `LumberProgressHandler.java`：
- 解析 payload：`session_id`, `tree_id`, `progress`, `display_name`
- 写入同一个 `GatheringProgressStore`
- HUD 复用 `GatheringProgressHud`

**注册**：
```java
handlers.put("lumber_progress", new LumberProgressHandler());
```

### D. GatheringProgressStore

```java
// client/.../gathering/GatheringProgressStore.java
public final class GatheringProgressStore {
    private static volatile GatheringProgressSnapshot current = GatheringProgressSnapshot.EMPTY;
    public static GatheringProgressSnapshot snapshot() { return current; }
    public static void update(String sessionId, String displayName, float progress) { ... }
    public static void clear(String sessionId) { ... }
}
```

`GatheringProgressHud.buildCommands()` 从此 store 读当前进度渲染。

---

## P1 — 饱和测试

1. `trade_offer` payload 到达 → `SocialServerDataHandler.handleTradeOffer()` 被调用
2. `mining_progress` payload 到达 → `GatheringProgressStore` 更新 → HUD 显示进度条
3. `lumber_progress` payload 到达 → store 更新 → HUD 显示进度条
4. 采矿完成（progress=1.0）→ HUD 进度条消失
5. 多个 session 并行 → 只显示最新活跃 session
6. 断线重连 → store 清空（无残留进度条）
7. 未注册类型 → `ServerDataRouter` 走 unknown 分支不崩溃（回归）

## Finish Evidence

### 落地清单

- P0 / trade_offer 注册：`client/src/main/java/com/bong/client/network/ServerDataRouter.java` 注册 `trade_offer` 到 `SocialServerDataHandler`。
- P0 / mining_progress 接入：`MiningProgressHandler` + `GatheringProgressPayloadReader` 将 `mining_progress` 写入 `GatheringSessionStore`，复用 `GatheringProgressHud` 渲染。
- P0 / lumber_progress 接入：`LumberProgressHandler` + `GatheringProgressPayloadReader` 将 `lumber_progress` 写入同一 gathering store/HUD。
- P0 / 断线清理：`BongNetworkHandler` disconnect hook 调用 `GatheringSessionStore.clearOnDisconnect()`，避免重连残留进度条。
- P1 / 饱和测试：`GatheringProgressHandlerTest` 覆盖 mining/lumber 更新、完成清理、并行 session 最新优先、invalid payload no-op、断线清理；`ServerDataRouterTest` 固定注册表；`SocialServerDataHandlerTest` 改为经默认 router 验证 `trade_offer` 分发。

### 关键 commit

- `863afd1f5` · 2026-05-13 · `feat(client): 补全三条采集与交易接线`

### 测试结果

- `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew --no-daemon --console=plain test --tests "com.bong.client.network.GatheringProgressHandlerTest" --tests "com.bong.client.network.ServerDataRouterTest" --tests "com.bong.client.network.SocialServerDataHandlerTest"`：通过。
- `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew --no-daemon --console=plain test build`：通过，`1394 tests / 0 failures / 0 errors / 0 skipped`。
- `git diff --check`：通过。

### 跨仓库核验

- server：已存在 `ServerDataPayloadV1::MiningProgress` / `ServerDataPayloadV1::LumberProgress`、`send_mining_progress_to_client()`、`send_lumber_progress_to_client()`；本 plan 未改 server。
- client：`ServerDataRouter.createDefault()` 现在注册 `trade_offer`、`mining_progress`、`lumber_progress`；`SocialServerDataHandler.handleTradeOffer()`、`MiningProgressHandler`、`LumberProgressHandler`、`GatheringSessionStore`、`GatheringProgressHud` 全链路有测试覆盖。
- agent：无新增/修改。

### 遗留 / 后续

- 无。本次实现以当前 server schema 为准：`mining_progress` 使用 `session_id/ore_pos/progress/interrupted/completed`，`lumber_progress` 使用 `session_id/log_pos/progress/interrupted/completed/detail`；client 额外兼容 `display_name/mineral_id/tree_id` 作为未来 label 来源。
