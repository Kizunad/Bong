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
