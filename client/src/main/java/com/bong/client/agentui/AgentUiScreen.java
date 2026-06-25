package com.bong.client.agentui;

import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.ParentComponent;
import io.wispforest.owo.ui.parsing.UIModel;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.sound.SoundCategory;
import net.minecraft.sound.SoundEvents;
import net.minecraft.text.Text;
import net.minecraft.util.math.random.Random;
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
 * plan-agent-ui-data-v1 P1/P3 — 天道 Agent 动态 UI 面板。
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
 *
 * <p>P3 VFX（plan §5）：
 * <ul>
 *   <li>{@link #init()} 注册 {@link AgentUiVfxState} 到 {@link AgentUiVfxStore}（供 BongHudOrchestrator 生成覆层）</li>
 *   <li>播放 {@code ambient.soul_speed_loop}（pitch 0.6，vol 0.3）</li>
 *   <li>在玩家附近 burst 生成 8-12 个 {@code BongSpriteParticle}（#4A2A2A，lifetime 40t）</li>
 *   <li>天道启示面板（isTiandaoRevelation=true）额外注册 vignette+shake（由 {@link AgentUiVfxPlanner} 处理）</li>
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

    /** P3 VFX：粒子颜色 #4A2A2A（暗红灵气底色） */
    static final int VFX_PARTICLE_COLOR = 0x4A2A2A;

    /** P3 VFX：粒子 burst 数量 */
    static final int VFX_PARTICLE_COUNT = 10;

    /** P3 VFX：粒子 lifetime ticks */
    static final int VFX_PARTICLE_LIFETIME_TICKS = 40;

    private final String requestId;
    private final UIModel model;
    /**
     * 本地超时 tick 绝对值（= 创建时的 tick + timeout_ticks + grace）。
     * 0 = 不启用本地超时（XML 解析失败 fallback 时无需倒计时）。
     */
    private final long localExpireTick;
    /** 是否为 parse_error fallback 面板（会在 init 时发 parse_error response）。 */
    private final boolean isFallback;
    /**
     * P3：是否为天道启示面板（realm_gate=5，通灵境专属）。
     * true → init() 额外注册 vignette+shake VFX。
     */
    private final boolean isTiandaoRevelation;
    /** 是否已发送过 close signal（防重入）。 */
    private volatile boolean closed = false;

    private AgentUiScreen(
        String requestId,
        UIModel model,
        long localExpireTick,
        boolean isFallback,
        boolean isTiandaoRevelation
    ) {
        super(Text.literal("天道"));//  MC title bar（不显示 in MC screen）
        this.requestId = Objects.requireNonNull(requestId, "requestId");
        this.model = Objects.requireNonNull(model, "model");
        this.localExpireTick = localExpireTick;
        this.isFallback = isFallback;
        this.isTiandaoRevelation = isTiandaoRevelation;
    }

    /**
     * 从 server 推送的 XML 字符串创建动态面板。
     *
     * @param requestId           server 下发的 request_id
     * @param xmlPayload          server 下发的 sanitized XML（需含 {@code <owo-ui><components>...} 包裹层）
     * @param timeoutTicks        server 下发的超时 ticks
     * @param currentTick         当前游戏 tick（用于计算本地超时阈值）
     * @return 动态面板，或 fallback 面板（永不返回 null）
     */
    public static AgentUiScreen create(
        String requestId,
        String xmlPayload,
        int timeoutTicks,
        long currentTick
    ) {
        return create(requestId, xmlPayload, timeoutTicks, currentTick, false);
    }

    /**
     * P3：从 server 推送的 XML 字符串创建动态面板，支持指定面板类型（天道启示专属 VFX）。
     *
     * @param requestId           server 下发的 request_id
     * @param xmlPayload          server 下发的 sanitized XML（需含 {@code <owo-ui><components>...} 包裹层）
     * @param timeoutTicks        server 下发的超时 ticks
     * @param currentTick         当前游戏 tick（用于计算本地超时阈值）
     * @param isTiandaoRevelation 是否为天道启示面板（realm_gate=5，通灵境专属）
     * @return 动态面板，或 fallback 面板（永不返回 null）
     */
    public static AgentUiScreen create(
        String requestId,
        String xmlPayload,
        int timeoutTicks,
        long currentTick,
        boolean isTiandaoRevelation
    ) {
        Objects.requireNonNull(requestId, "requestId");
        UIModel model = tryLoadModel(xmlPayload);
        if (model != null) {
            long expireTick = currentTick + timeoutTicks + LOCAL_TIMEOUT_GRACE_TICKS;
            return new AgentUiScreen(requestId, model, expireTick, false, isTiandaoRevelation);
        }
        // XML 解析失败 → fallback（localExpireTick=0 禁用倒计时）
        LOGGER.warn("[bong][agent_ui] XML 解析失败，使用静态 fallback 面板 request_id={}", requestId);
        UIModel fallbackModel = loadFallbackModel();
        return new AgentUiScreen(requestId, fallbackModel, 0L, true, false);
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

    /**
     * P3 VFX — 面板打开时触发：
     * <ol>
     *   <li>注册 {@link AgentUiVfxState} 到 {@link AgentUiVfxStore}（供 HUD planner 读取）</li>
     *   <li>播放 {@code ambient.soul_speed_loop}（pitch 0.6，vol 0.3，无空间衰减）</li>
     *   <li>在玩家附近 burst 生成 {@value VFX_PARTICLE_COUNT} 个暗红粒子（从面板外围向内聚拢感）</li>
     * </ol>
     *
     * <p>所有 MC 调用均做 null 防护（单元测试环境无 client）。
     */
    @Override
    public void init() {
        super.init();

        // ── 注册 VFX 状态（fade-in + tiandao 特有效果）────────────────────────
        AgentUiVfxStore.setActive(new AgentUiVfxState(System.currentTimeMillis(), isTiandaoRevelation));

        // ── 非 fallback 时播放音效 + 粒子（fallback 时静默，避免 parse_error 也触发）────
        if (!isFallback) {
            playOpenSound();
            spawnOpenParticles();
        }
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
     * ESC 关闭 → 发 dismissed response + 清除 P3 VFX 状态。
     *
     * <p>{@code super.close()} 调用 {@code MinecraftClient.setScreen(null)}；
     * 单元测试环境中 {@code client} 为 null，必须加护卫，否则 NPE。
     */
    @Override
    public void close() {
        if (!closed) {
            closed = true;
            sendResponse("dismissed", Map.of());
        }
        AgentUiStore.clearIfActive(this);
        AgentUiVfxStore.clear();
        if (client != null) {
            super.close();
        }
    }

    // ─── 内部 helpers ─────────────────────────────────────────────────────────

    /** 关闭面板但不发 response（server close 信号 / 本地超时 / fallback dismiss）+ 清除 P3 VFX 状态。 */
    private void closeWithoutResponse() {
        if (!closed) {
            closed = true;
        }
        AgentUiStore.clearIfActive(this);
        AgentUiVfxStore.clear();
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

    /** P3：是否为天道启示面板（测试 + AgentUiVfxState 使用）。 */
    public boolean isTiandaoRevelation() {
        return isTiandaoRevelation;
    }

    // ─── 测试 seam（package-private）────────────────────────────────────────

    /**
     * 单元测试用：模拟玩家点击指定 ID 的按钮，触发 {@code button_click} 响应逻辑。
     * 不依赖 MC OwoUI 组件树，供 {@code AgentUiScreenProtocolTest} 捕获 sentPayloads。
     */
    void simulateButtonClickForTests(String buttonId) {
        onButtonClicked(buttonId);
    }

    /**
     * 单元测试用：查询 {@code closed} 标志当前状态。
     * 用于断言幂等性（"二次操作不再发包"）。
     */
    boolean isClosedForTests() {
        return closed;
    }

    /**
     * 单元测试用：是否为 parse_error fallback 面板。
     * valid XML 创建的 screen 应返回 false；空/畸形 XML 走 fallback 路径的 screen 返回 true。
     * 用于区分 "UIModel.load 真正接受了 XML" vs "静默降级到 fallback"。
     */
    boolean isFallbackForTests() {
        return isFallback;
    }

    // ─── P3 VFX helpers ──────────────────────────────────────────────────────

    /**
     * 播放面板打开音效：{@code ambient.soul_sand_valley.loop}（灵魂沙谷环境音，
     * 接近 plan §5 所指 {@code ambient.soul_speed_loop} 的氛围感），pitch 0.6，volume 0.3。
     *
     * <p>MC 1.20.1 Yarn 中 AMBIENT_SOUL_SAND_VALLEY_LOOP 为
     * {@code RegistryEntry$Reference<SoundEvent>}，需调用 {@code .value()}。
     * MC 环境不可用时静默忽略。
     */
    private void playOpenSound() {
        try {
            MinecraftClient mc = MinecraftClient.getInstance();
            if (mc == null || mc.player == null || mc.world == null) {
                return;
            }
            mc.world.playSound(
                mc.player,
                mc.player.getBlockPos(),
                SoundEvents.AMBIENT_SOUL_SAND_VALLEY_LOOP.value(),
                SoundCategory.AMBIENT,
                /* volume */ 0.3f,
                /* pitch  */ 0.6f
            );
        } catch (Exception e) {
            LOGGER.debug("[bong][agent_ui] playOpenSound 失败（测试环境预期）：{}", e.getMessage());
        }
    }

    /**
     * P3 VFX：在玩家附近 burst 生成 {@value VFX_PARTICLE_COUNT} 个 {@code BongSpriteParticle}。
     *
     * <p>颜色 {@code #4A2A2A}（暗红灵气底色），lifetime 40t，
     * 速度方向随机向内（从玩家上方 scatter）模拟「四角聚拢」感。
     * MC 环境不可用时静默忽略。
     */
    private void spawnOpenParticles() {
        try {
            MinecraftClient mc = MinecraftClient.getInstance();
            if (mc == null || mc.player == null || mc.world == null) {
                return;
            }
            ClientWorld world = mc.world;
            net.minecraft.entity.player.PlayerEntity player = mc.player;
            Random rng = Random.create();

            float r = ((VFX_PARTICLE_COLOR >> 16) & 0xFF) / 255.0f;
            float g = ((VFX_PARTICLE_COLOR >> 8)  & 0xFF) / 255.0f;
            float b = ((VFX_PARTICLE_COLOR)        & 0xFF) / 255.0f;

            for (int i = 0; i < VFX_PARTICLE_COUNT; i++) {
                // 在玩家周围 ±1.5 范围内 scatter（模拟四角聚拢）
                double ox = (rng.nextDouble() - 0.5) * 3.0;
                double oy = rng.nextDouble() * 2.5;
                double oz = (rng.nextDouble() - 0.5) * 3.0;
                double px = player.getX() + ox;
                double py = player.getY() + oy;
                double pz = player.getZ() + oz;

                // 速度方向：朝玩家中心聚拢（inward）
                double vx = -ox * 0.03;
                double vy = -oy * 0.01;
                double vz = -oz * 0.03;

                // 使用内置 ENTITY_EFFECT 粒子（可设颜色），避免自定义粒子类型注册
                // x/y/z 作为 count=0 时的 speed 参数，data 通过 ENTITY_EFFECT 的 r/g/b 编码
                world.addParticle(
                    new net.minecraft.particle.DustParticleEffect(
                        new org.joml.Vector3f(r, g, b),
                        /* scale */ 1.2f
                    ),
                    px, py, pz,
                    vx, vy, vz
                );
            }
        } catch (Exception e) {
            LOGGER.debug("[bong][agent_ui] spawnOpenParticles 失败（测试环境预期）：{}", e.getMessage());
        }
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
