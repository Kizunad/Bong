# 顿悟邀约 UI (Insight Offer Screen)

服务端推送顿悟邀约 (`InsightOfferViewModel`) 时弹出的模态弹窗。对应 `docs/plan-cultivation-v1.md §5` 设计。

```
┌──────────────────────────────────────────────────────────┐
│              ◇ 心 有 所 感 ◇                              │
│       【触发】首次突破到引气境                              │
│                                                          │
│   境界: 引气境 (3 正经)   心境: 0.78   剩余顿悟额度: 2/2   │
│   ⏳ 53s                                                  │
│   ─────────────────────────────────────────────          │
│                                                          │
│   ┌────────────┐  ┌────────────┐  ┌────────────┐        │
│   │类 E · 突破  │  │类 C · 心境  │  │类 G · 感知  │        │
│   │ 下次冲关稳 │  │ 闭关心如止 │  │ 灵气浓淡见 │        │
│   │ +5% rate   │  │ immune     │  │ unlock_zone│        │
│   │ ✦ 你已知…  │  │ ✦ 闭关时…  │  │ ✦ 你能感…  │        │
│   │ → 保下一关 │  │ → 突破基线 │  │ → 战略侦察 │        │
│   └────────────┘  └────────────┘  └────────────┘        │
│                                                          │
│         [ 心未契机 ]  拒绝, 不消耗额度                     │
└──────────────────────────────────────────────────────────┘
```

## 文件结构

```
insight/
├── InsightCategory.java          # 7 类 (A-G) 枚举 + 类别色
├── InsightChoice.java            # 单候选 record (id/类别/标题/数值/flavor/风格)
├── InsightOfferViewModel.java    # 一次邀约 (offerId/trigger/realm/quota/expires/choices)
├── InsightDecision.java          # 玩家决定 (CHOSEN/DECLINED/TIMED_OUT)
├── InsightChoiceDispatcher.java  # 决定回传接口 (LOGGING 默认实现)
├── InsightOfferStore.java        # volatile 单 slot + listeners + dispatcher
├── InsightOfferScreen.java       # owo-lib 主屏 (倒计时 / 卡片点击 / ESC 拒绝)
└── InsightOfferScreenBootstrap.java
                                  # 监听 store 自动开关屏
```

## 交互

- **点候选卡** → `InsightDecision.chosen(...)` → dispatcher → 关屏
- **点"心未契机"** → `InsightDecision.declined(...)` → dispatcher → 关屏 (额度不消耗)
- **倒计时归零** → `InsightDecision.timedOut(...)` → 关屏
- **ESC** → 等价于"心未契机" (`close()` 重写)
- **不暂停游戏** (`shouldPause() = false`) — 顿悟期间世界继续推进

## 状态流

```
Server (待接入)            InsightOfferStore                Screen
─────────────              ─────────────────              ──────────
publish offer ─────────────► replace(vm) ───listener──► open Screen
                                                              ▼
                                                        玩家选择 / 拒绝 / 超时
                                                              ▼
                            submit(decision)              dispatch(decision)
                                  │                              │
                                  └───► dispatcher ──────► (network / log)
                                  │
                            replace(null) ───listener──► close Screen
```

## 接入服务端

后续在 `BongNetworkHandler` 实现 `bong:insight_request` payload 收发：

1. 收到 server payload → 反序列化为 `InsightOfferViewModel` → `InsightOfferStore.replace(...)`
2. 启动时 `InsightOfferStore.setDispatcher(new NetworkInsightDispatcher(...))` 替换 LOGGING；
   该 dispatcher 把 `InsightDecision` 序列化后通过 CustomPayload 发回 server。

UI 层无需改动。

## 设计约束 (与 plan §5.2 对齐)

- 客户端**只展示**，不校验数值上限 / 白名单 — 校验在服务端 Arbiter
- 客户端**不解释** `effectSummary` 的语义 — 文本由 server/agent 决定
- 卡片类别色 (`InsightCategory.accentArgb()`) 是**仅展示用**的视觉提示，不影响逻辑
- 每个 offer 至多 4 个候选 (3 选项 + 兜底)，多于 4 则视为非法
