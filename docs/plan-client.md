# Client 路线详细计划（Java / Fabric 1.20.1）

> 从 CustomPayload 接收器推进到修仙沙盒的沉浸式视觉表现层。
> 纯 Client-Side Mod，不修改注册表，保持极致轻量。

---

## 当前代码结构

```
client/src/main/java/com/bong/client/
├── BongClient.java             # Mod 入口 (ClientModInitializer)
├── BongNetworkHandler.java     # CustomPayload 监听 (bong:server_data)
└── BongHud.java                # 简单 HUD "Bong Client Connected"

client/src/main/resources/
├── fabric.mod.json
└── bong-client.mixins.json     # 空
```

---

## M1 — 天道闭环

### C1. Narration 频道监听与渲染

**目标**：接收 server 转发的天道叙事，按风格分类渲染到聊天栏。

**改动**：`BongNetworkHandler.java`

**当前状态**：只监听 `bong:server_data` 频道，解析 JSON 打印原始内容。

**改造**：

```java
// 注册新频道（或复用 bong:server_data 加 type 字段区分）
// 建议方案：server 发不同 type 的 JSON 到 bong:server_data

public static void handleServerData(MinecraftClient client, PacketByteBuf buf) {
    String json = buf.readString();
    JsonObject obj = JsonParser.parseString(json).getAsJsonObject();
    String type = obj.get("type").getAsString();

    switch (type) {
        case "narration" -> handleNarration(client, obj);
        case "zone_info" -> handleZoneInfo(client, obj);  // M2
        case "event_alert" -> handleEventAlert(client, obj); // M2
        default -> {} // ignore unknown
    }
}
```

**Narration 渲染**：

```java
private static void handleNarration(MinecraftClient client, JsonObject obj) {
    JsonArray narrations = obj.getAsJsonArray("narrations");
    for (JsonElement el : narrations) {
        JsonObject n = el.getAsJsonObject();
        String style = n.get("style").getAsString();
        String text = n.get("text").getAsString();

        // MC 格式化颜色码
        String formatted = switch (style) {
            case "system_warning" -> "§c§l[天道警示]§r §c" + text;
            case "perception"     -> "§7[感知]§r §7" + text;
            case "narration"      -> "§f[叙事]§r §f" + text;
            case "era_decree"     -> "§6§l[§e时代§6§l]§r §6" + text;
            default               -> "§f" + text;
        };

        // 发送到聊天栏
        client.execute(() -> {
            if (client.player != null) {
                client.player.sendMessage(Text.literal(formatted), false);
            }
        });
    }
}
```

**验证**：
- Server 运行 + Agent 运行
- 玩家进入游戏，30 秒内聊天栏出现彩色天道消息

---

### C2. Narration HUD Toast（可选增强）

**目标**：重要 narration（system_warning / era_decree）不仅在聊天栏，还在屏幕中央弹出醒目提示。

**新增**：`BongToast.java`

```java
public class BongToast {
    private static String currentToast = null;
    private static long toastExpiry = 0;
    private static int toastColor = 0xFFFFFF;

    public static void show(String text, int color, int durationMs) {
        currentToast = text;
        toastColor = color;
        toastExpiry = System.currentTimeMillis() + durationMs;
    }

    // 在 HudRenderCallback 中调用
    public static void render(DrawContext context, int scaledWidth, int scaledHeight) {
        if (currentToast == null || System.currentTimeMillis() > toastExpiry) {
            currentToast = null;
            return;
        }

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        int width = textRenderer.getWidth(currentToast);
        int x = (scaledWidth - width) / 2;
        int y = scaledHeight / 4; // 屏幕上方 1/4 处

        // 半透明背景
        context.fill(x - 4, y - 4, x + width + 4, y + 12, 0x88000000);
        // 文字
        context.drawText(textRenderer, currentToast, x, y, toastColor, true);
    }
}
```

**触发**：
- `system_warning` → Toast 红色，持续 5 秒
- `era_decree` → Toast 金色，持续 8 秒
- `perception` / `narration` → 不触发 Toast，只在聊天栏

---

### C3. 天象视觉反馈（可选增强）

**目标**：天劫等事件时有简单的视觉暗示。

**实现方式**：不需要 Shader，用原版手段：

```
天劫 (system_warning)：
  → 持续 3 秒每 tick 摇晃玩家视角（bobbing 叠加微量偏移）
  → 用 Mixin 注入 GameRenderer.renderWorld，添加微小 pitch 抖动
  → 或更简单：连续发送 Title 消息模拟闪烁

灵气变化 (perception)：
  → 短暂改变 fog color（Mixin FogRenderer）
  → 灵气上升 → 偏蓝绿，灵气下降 → 偏暗红
  → 持续 2 秒后渐变恢复

时代宣言 (era_decree)：
  → 全屏 Title：金色大字，渐入渐出
  → client.player.sendMessage 配合 Title 包
```

**注意**：Mixin 是可选的，M1 可以先只做 Title/聊天栏，M2 再加 Mixin 增强。

---

## M2 — 有意义的世界

### C4. 区域 HUD

**目标**：玩家进入不同区域时，屏幕显示区域名和灵气浓度。

**实现**：Server 在玩家跨区域时发送 `zone_info` 类型的 CustomPayload。

**新增**：`BongZoneHud.java`

```java
public class BongZoneHud {
    private static String zoneName = "";
    private static double spiritQi = 0;
    private static int dangerLevel = 0;
    private static long zoneChangeTime = 0;

    public static void update(String name, double qi, int danger) {
        zoneName = name;
        spiritQi = qi;
        dangerLevel = danger;
        zoneChangeTime = System.currentTimeMillis();
    }

    public static void render(DrawContext context, int scaledWidth, int scaledHeight) {
        if (zoneName.isEmpty()) return;

        TextRenderer tr = MinecraftClient.getInstance().textRenderer;

        // 区域名 + 淡入效果（进入后 2 秒渐隐）
        long elapsed = System.currentTimeMillis() - zoneChangeTime;
        boolean showBig = elapsed < 2000;

        if (showBig) {
            // 大字居中显示区域名
            String display = "— " + zoneName + " —";
            int w = tr.getWidth(display);
            int alpha = elapsed < 1500 ? 255 : (int)(255 * (2000 - elapsed) / 500.0);
            int color = (alpha << 24) | 0xFFD700; // 金色
            context.drawText(tr, display, (scaledWidth - w) / 2, scaledHeight / 3, color, true);
        }

        // 常驻小字：左上角灵气条
        String qiBar = qiToBar(spiritQi);
        String dangerText = "☠".repeat(dangerLevel);
        context.drawText(tr, "§b灵气 " + qiBar, 4, 20, 0xFFFFFF, false);
        if (dangerLevel > 0) {
            context.drawText(tr, "§c危险 " + dangerText, 4, 32, 0xFFFFFF, false);
        }
    }

    private static String qiToBar(double qi) {
        int filled = (int)(qi * 10);
        return "§a" + "█".repeat(filled) + "§8" + "█".repeat(10 - filled);
    }
}
```

**Server 侧配合**：
- 玩家 Position 变化时检查是否跨 zone
- 跨 zone 时发 `{ type: "zone_info", zone: "blood_valley", spirit_qi: 0.42, danger_level: 3 }`

---

### C5. CustomPayload 路由器

**目标**：统一的消息分发框架，便于后续扩展。

**改造**：`BongNetworkHandler.java`

```java
public class BongNetworkHandler {
    // Handler 注册表
    private static final Map<String, BiConsumer<MinecraftClient, JsonObject>> HANDLERS = Map.of(
        "narration",   NarrationHandler::handle,
        "zone_info",   ZoneInfoHandler::handle,
        "event_alert", EventAlertHandler::handle,
        "welcome",     WelcomeHandler::handle
    );

    public static void register() {
        ClientPlayNetworking.registerGlobalReceiver(
            new Identifier("bong", "server_data"),
            (client, handler, buf, sender) -> {
                String json = buf.readString(32767);
                try {
                    JsonObject obj = JsonParser.parseString(json).getAsJsonObject();
                    String type = obj.has("type") ? obj.get("type").getAsString() : "unknown";
                    BiConsumer<MinecraftClient, JsonObject> h = HANDLERS.get(type);
                    if (h != null) {
                        client.execute(() -> h.accept(client, obj));
                    }
                } catch (Exception e) {
                    // ignore malformed
                }
            }
        );
    }
}
```

每种消息类型一个 Handler 类，职责清晰。

---

## M3 — 修仙体验

### C6. 修仙 UI 面板（owo-ui）

**目标**：按键打开修仙面板，显示境界、真元池、karma 等。

**新增**：`ui/CultivationScreen.java`

**触发**：按 `K` 键打开（KeyBinding 注册）

**面板内容**：

```
┌─────────────────────────────────┐
│         修 仙 面 板              │
├─────────────────────────────────┤
│  境界: 练气三层                  │
│                                  │
│  真元: ████████░░ 78/100         │
│                                  │
│  因果 (karma): +0.20             │
│  [═══════●════] 善 ←→ 恶        │
│                                  │
│  综合实力: 0.35                  │
│  ├ 战斗: 0.20                   │
│  ├ 财富: 0.40                   │
│  ├ 社交: 0.65                   │
│  └ 领地: 0.10                   │
│                                  │
│  当前区域: 青云峰                │
│  灵气浓度: ████████░░            │
└─────────────────────────────────┘
```

**数据来源**：Server 通过 CustomPayload 定期（每 5 秒）发送 `{ type: "player_state", realm: "qi_refining_3", spirit_qi: 78, ... }`

**owo-ui 实现**：
```java
// 使用 owo-ui 的 FlowLayout
public class CultivationScreen extends BaseOwoScreen<FlowLayout> {
    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.child(Components.label(Text.literal("境界: " + realm)));
        root.child(buildQiBar());
        root.child(buildKarmaSlider());
        root.child(buildPowerBreakdown());
    }
}
```

---

### C7. 动态 UI 下发（可选/远期）

**目标**：Server 可以下发 UI 布局 XML，Client 动态渲染。

**实现**（tech-audit.md 已验证可行）：

```java
// Server 发: { type: "ui_open", xml: "<flow-layout ...>...</flow-layout>" }

// Client 收到后:
InputStream is = new ByteArrayInputStream(xml.getBytes(StandardCharsets.UTF_8));
UIModel model = UIModel.load(is);
OwoUIAdapter<?> adapter = model.createAdapter(FlowLayout.class, screen);
screen.uiAdapter = adapter;
```

**安全**：
- XML 解析需配置 XXE 防护
- 预注册允许的 UI 组件白名单
- 限制 XML 大小（< 10KB）

---

## 文件规划总览

```
client/src/main/java/com/bong/client/
├── BongClient.java                # Mod 入口
├── network/
│   ├── BongNetworkHandler.java    # CustomPayload 路由器 (C5)
│   ├── NarrationHandler.java      # 天道叙事处理 (C1)
│   ├── ZoneInfoHandler.java       # 区域信息处理 (C4)
│   ├── EventAlertHandler.java     # 事件警报处理 (M2)
│   └── WelcomeHandler.java        # 欢迎消息
├── hud/
│   ├── BongHud.java               # 总 HUD 渲染入口
│   ├── BongToast.java             # 居中 Toast 提示 (C2)
│   └── BongZoneHud.java           # 区域信息 HUD (C4)
├── visual/
│   └── SkyEffects.java            # 天象视觉（Mixin, M2+）
└── ui/
    └── CultivationScreen.java     # 修仙面板 (C6, M3)
```

---

## 开发顺序建议

```
M1 顺序：
  C1 Narration 渲染（核心，先做）
  C2 Toast 提示（简单增强，可并行）
  C3 天象视觉（可选，M1 先跳过也可）

M2 顺序：
  C5 Payload 路由器（先重构，后续都依赖）
  C4 区域 HUD（依赖 C5 + Server 的 zone_info 下发）

M3 顺序：
  C6 修仙 UI 面板（依赖 Server 的 player_state 下发）
  C7 动态 UI（远期，可选）
```

---

## 构建与测试

```bash
# 编译
cd client && ./gradlew build

# 开发态测试（WSLg）
sdk use java 17.0.18-amzn
./gradlew runClient
# MC 窗口 → 多人游戏 → localhost:25565

# 单元测试
./gradlew test
```

**手动测试快捷方式**：
- `redis-cli PUBLISH bong:agent_narrate '{"v":1,"narrations":[{"scope":"broadcast","text":"天道测试消息","style":"system_warning"}]}'`
- Server 收到后转发给所有 client
- Client 聊天栏应显示红色 `[天道警示] 天道测试消息`

---

## 依赖清单

| 依赖 | 版本 | 用途 |
|------|------|------|
| Fabric Loader | 0.16.10 | Mod 加载 |
| Fabric API | 0.92.3+1.20.1 | Networking, Rendering |
| owo-lib | 0.11.2+1.20 | UI 框架 (M3) |
| Minecraft | 1.20.1 | 基座 |
| Yarn Mappings | 1.20.1+build.10 | 反编译映射 |
