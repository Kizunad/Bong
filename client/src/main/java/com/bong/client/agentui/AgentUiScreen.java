package com.bong.client.agentui;

import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.ParentComponent;
import io.wispforest.owo.ui.parsing.UIModel;
import net.minecraft.text.Text;
import org.jetbrains.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.xml.parsers.ParserConfigurationException;
import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.Objects;

/**
 * plan-agent-ui-data-v1 P1 — 天道 Agent 动态 UI 面板。
 *
 * <p>从 server 下发的 XML 字节流调用 {@link UIModel#load(java.io.InputStream)} 构建 OwoUI 组件树；
 * 所有 {@code <button id="...">} 绑定 onClick → 发送 {@code agent_ui_response} CustomPayload。
 *
 * <p>关闭语义：
 * <ul>
 *   <li>ESC → {@code dismissed} 响应</li>
 *   <li>本地超时（timeout_ticks + 20t 宽限）→ 仅本地关闭，<b>不发任何 response</b>（server TimedOut 权威）</li>
 *   <li>server {@code agent_ui_close} 信号 → 调用 {@link #receiveCloseSignal} 关闭，不发任何 response</li>
 *   <li>XML 解析失败 → fallback 静态面板 + 发 {@code parse_error} response</li>
 * </ul>
 */
public final class AgentUiScreen extends BaseOwoScreen<FlowLayout> {

    private static final Logger LOGGER = LoggerFactory.getLogger(AgentUiScreen.class);

    /** 本地宽限 tick（= 20t = 1s），确保 server close 信号先于 client 本地超时关闭。 */
    static final int LOCAL_TIMEOUT_GRACE_TICKS = 20;

    /** parse_error 时展示的静态 fallback XML。 */
    static final String FALLBACK_XML =
        "<owo-ui>"
        + "<components>"
        + "<flow-layout direction=\"vertical\" gap=\"4\">"
        + "<label>天道信号紊乱，法则碎片无法解析</label>"
        + "<button id=\"fallback_dismiss\">关闭</button>"
        + "</flow-layout>"
        + "</components>"
        + "</owo-ui>";

    private final String requestId;
    private final UIModel model;
    /**
     * 本地超时 tick 绝对值（= 创建时的 tick + timeout_ticks + grace）。
     * 0 = 不启用本地超时（XML 解析失败 fallback 时无需倒计时）。
     */
    private final long localExpireTick;
    /** 是否为 parse_error fallback 面板（会在 init 时发 parse_error response）。 */
    private final boolean isFallback;
    /** 是否已发送过 close signal（防重入）。 */
    private volatile boolean closed = false;

    private AgentUiScreen(String requestId, UIModel model, long localExpireTick, boolean isFallback) {
        super(Text.literal("天道"));//  MC title bar（不显示 in MC screen）
        this.requestId = Objects.requireNonNull(requestId, "requestId");
        this.model = Objects.requireNonNull(model, "model");
        this.localExpireTick = localExpireTick;
        this.isFallback = isFallback;
    }

    /**
     * 从 server 推送的 XML 字符串创建动态面板。
     *
     * @param requestId    server 下发的 request_id
     * @param xmlPayload   server 下发的 sanitized XML（需含 {@code <owo-ui><components>...} 包裹层）
     * @param timeoutTicks server 下发的超时 ticks
     * @param currentTick  当前游戏 tick（用于计算本地超时阈值）
     * @return 动态面板，或 fallback 面板（永不返回 null）
     */
    public static AgentUiScreen create(
        String requestId,
        String xmlPayload,
        int timeoutTicks,
        long currentTick
    ) {
        Objects.requireNonNull(requestId, "requestId");
        UIModel model = tryLoadModel(xmlPayload);
        if (model != null) {
            long expireTick = currentTick + timeoutTicks + LOCAL_TIMEOUT_GRACE_TICKS;
            return new AgentUiScreen(requestId, model, expireTick, false);
        }
        // XML 解析失败 → fallback（localExpireTick=0 禁用倒计时）
        LOGGER.warn("[bong][agent_ui] XML 解析失败，使用静态 fallback 面板 request_id={}", requestId);
        UIModel fallbackModel = loadFallbackModel();
        return new AgentUiScreen(requestId, fallbackModel, 0L, true);
    }

    // ─── OwoUI lifecycle ─────────────────────────────────────────────────────

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return model.createAdapter(FlowLayout.class, this);
    }

    @Override
    protected void build(FlowLayout rootComponent) {
        if (isFallback) {
            // parse_error fallback：立即发 parse_error response，只绑定关闭按钮
            sendResponse("parse_error", Map.of());
            wireButton(rootComponent, "fallback_dismiss", () -> closeWithoutResponse());
            return;
        }
        wireAllButtons(rootComponent);
    }

    // ─── 外部信号接口 ─────────────────────────────────────────────────────────

    /**
     * 收到 server {@code agent_ui_close} 信号后调用。
     * 关闭面板，不发任何 response（server 已发布终态 Redis）。
     */
    public void receiveCloseSignal() {
        closeWithoutResponse();
    }

    /**
     * 游戏 tick 回调（由 BongClient tick 或 screen tick 驱动）。
     * 本地超时到期时仅关闭面板，不发任何 response。
     */
    public void tickLocalTimeout(long currentTick) {
        if (localExpireTick > 0 && currentTick >= localExpireTick) {
            LOGGER.debug("[bong][agent_ui] 本地超时 request_id={} currentTick={} expireTick={}",
                requestId, currentTick, localExpireTick);
            closeWithoutResponse();
        }
    }

    /** 本 screen 的 request_id（AgentUiStore 查找匹配用）。 */
    public String requestId() {
        return requestId;
    }

    // ─── Screen 关闭 ─────────────────────────────────────────────────────────

    /**
     * ESC 关闭 → 发 dismissed response。
     */
    @Override
    public void close() {
        if (!closed) {
            closed = true;
            sendResponse("dismissed", Map.of());
        }
        super.close();
    }

    // ─── 内部 helpers ─────────────────────────────────────────────────────────

    /** 关闭面板但不发 response（server close 信号 / 本地超时 / fallback dismiss）。 */
    private void closeWithoutResponse() {
        if (!closed) {
            closed = true;
        }
        if (client != null && client.currentScreen == this) {
            client.setScreen(null);
        }
    }

    /**
     * 遍历组件树，为所有 ButtonComponent 绑定 onPress handler。
     * 按钮 ID 来自 XML {@code id=} 属性；无 ID 按钮绑定关闭行为。
     */
    private void wireAllButtons(ParentComponent root) {
        wireButtonsRecursive(root);
    }

    private void wireButtonsRecursive(ParentComponent parent) {
        for (var child : parent.children()) {
            if (child instanceof ButtonComponent button) {
                String id = button.id();
                if (id != null && !id.isBlank()) {
                    final String buttonId = id;
                    button.onPress(b -> onButtonClicked(buttonId));
                } else {
                    button.onPress(b -> onButtonClicked(""));
                }
            }
            if (child instanceof ParentComponent pc) {
                wireButtonsRecursive(pc);
            }
        }
    }

    private void wireButton(ParentComponent root, String buttonId, Runnable action) {
        ButtonComponent button = root.childById(ButtonComponent.class, buttonId);
        if (button != null) {
            button.onPress(b -> action.run());
        }
    }

    private void onButtonClicked(String buttonId) {
        if (!closed) {
            closed = true;
            sendResponse("button_click", Map.of("button_id", buttonId));
            if (client != null && client.currentScreen == this) {
                client.setScreen(null);
            }
        }
    }

    private void sendResponse(String action, Map<String, String> params) {
        ClientRequestSender.sendAgentUiResponse(requestId, action, params);
    }

    // ─── XML loading helpers ──────────────────────────────────────────────────

    @Nullable
    private static UIModel tryLoadModel(String xmlPayload) {
        if (xmlPayload == null || xmlPayload.isBlank()) {
            return null;
        }
        try {
            byte[] bytes = xmlPayload.getBytes(StandardCharsets.UTF_8);
            try (ByteArrayInputStream stream = new ByteArrayInputStream(bytes)) {
                return UIModel.load(stream);
            }
        } catch (ParserConfigurationException | IOException | org.xml.sax.SAXException | RuntimeException e) {
            LOGGER.warn("[bong][agent_ui] UIModel.load 失败：{}", e.getMessage());
            return null;
        }
    }

    private static UIModel loadFallbackModel() {
        try {
            byte[] bytes = FALLBACK_XML.getBytes(StandardCharsets.UTF_8);
            try (ByteArrayInputStream stream = new ByteArrayInputStream(bytes)) {
                return UIModel.load(stream);
            }
        } catch (Exception e) {
            throw new IllegalStateException("fallback XML 解析失败，这是代码 bug", e);
        }
    }
}
