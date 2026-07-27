# plan-bughunt-botany-drag-release-lifecycle-v1（骨架）

> 一句话主题：在屏幕切换/会话结束时确定性终止 Botany 左键拖拽，避免 `MixinMouse` 漏收 release 后吞掉关屏后的首个左键 RELEASE。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 冻结 drag ownership 与 screen/session reset 语义 | ⬜ |
| P1 | 输入生命周期修复 + client 测试 | ⬜ |
| P2 | JDK 17 Gradle gate + 真机输入回归 | ⬜ |

## 接入面

- **进料**：`client/.../mixin/MixinMouse.java` LEFT press/release、`BotanyDragState`、botany session/screen 生命周期。
- **出料**：拖拽结束后输入继续交给 Minecraft/其他 HUD；只在 botany 真消费时 cancel。
- **共享类型 / event**：复用 `BotanyDragState` 与当前 input router；不另造第二 mouse state。
- **跨仓库契约**：纯 client，无 payload 变化。
- **worldview 锚点**：无新玩法；只是既有灵植 UI 输入完整性。
- **qi_physics 锚点**：不涉及。

## 当前证据（origin/main @ c625d5a5）

`client/src/main/java/com/bong/client/mixin/MixinMouse.java:101` 在 `currentScreen != null` 时早退，拖拽期间打开屏幕会漏掉 LEFT RELEASE；静态 `BotanyDragState.dragging` 因而保持 true。关屏后的下一次 release 会被 `onLeftButton(0)` 当成旧拖拽终止并返回 consumed，mixin 随即 cancel。

## 验收

1. 覆盖正常 press→drag→release、drag 中开屏、drag 中 session 替换/结束、断线、开屏期间非 botany click。
2. 任一生命周期终止后 `dragging=false`；关屏后的首个普通 click/release 不被吞。
3. release 重复到达幂等，非左键与已有盾牌右键逻辑不受影响。
4. JDK 17 运行 `cd client && ./gradlew test build`，并真机验证 botany panel 与普通 GUI 切换。

## 边界

- 不重写全局输入框架，不改变 botany 拖拽命中几何或 session 协议。
- 不把 screen-open 的所有 mouse event 交给 botany；只做 stale state reset/确定性 release。
