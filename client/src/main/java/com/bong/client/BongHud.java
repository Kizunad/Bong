package com.bong.client;

import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DefenseWindowStore;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.SpellVolumeStore;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.combat.UnlockedStylesStore;
import com.bong.client.combat.baomai.v3.BaomaiV3Hud;
import com.bong.client.combat.baomai.v4.CrackReadingOverlay;
import com.bong.client.visual.VoidErosionHudOverlay;
import com.bong.client.combat.baomai.v4.ResonanceLockMeterHud;
import com.bong.client.hud.BongHudOrchestrator;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.BotanyProjection;
import com.bong.client.hud.CombatHudSnapshot;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRuntimeContext;
import com.bong.client.hud.HudTextHelper;
import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.svg.HudRenderBackend;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.tiandao.TiandaoPresenceHudPlanner;
import com.bong.client.tiandao.TiandaoPresenceStore;
import com.bong.client.ui.ClientConnectionStatusStore;
import com.bong.client.ui.ScreenTransition;
import com.bong.client.ui.ScreenTransitionOverlay;
import net.minecraft.client.render.Camera;
import net.minecraft.util.math.Vec3d;
import com.bong.client.inventory.state.PhysicalBodyStore;
import com.bong.client.visual.EdgeDecalRenderer;
import com.bong.client.visual.InkWashVignetteRenderer;
import com.bong.client.visual.OverlayQuadRenderer;
import com.bong.client.visual.realm_vision.EdgeIndicatorCmd;
import com.bong.client.visual.realm_vision.PerceptionEdgeProjector;
import com.bong.client.visual.realm_vision.PerceptionEdgeRenderer;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;
import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.util.Identifier;
import net.minecraft.util.Util;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.function.Supplier;

public class BongHud {
    private static final int HUD_TEXT_MAX_WIDTH = 220;
    public static void render(DrawContext context, float tickDelta) {
        render(context, tickDelta, HudRenderBackend.NOOP);
    }

    /** Fabric 回调入口；具体表现后端由组合根注入，便于替换 SVG 实现。 */
    public static void render(DrawContext context, float tickDelta, HudRenderBackend backend) {
        MinecraftClient client = MinecraftClient.getInstance();
        long nowMillis = System.currentTimeMillis();
        ClientConnectionStatusStore.tick(Util.getMeasuringTimeMs(), nowMillis);

        // Tick cast-state + defense-window expiries so they self-clear each frame.
        CastStateStore.tick(nowMillis);
        DefenseWindowStore.tick(nowMillis);
        com.bong.client.tsy.ExtractStateStore.tick(nowMillis);
        // Open death/terminate screens when the server activates them.
        com.bong.client.combat.screen.CombatScreenOpener.tick();

        Screen currentScreen = client.currentScreen;
        if (currentScreen == null) {
            ScreenTransitionOverlay.render(context, client, ScreenTransition.nowMillis());
        }
        render(
            currentScreen,
            nowMillis,
            () -> captureHudFrameInput(client, nowMillis),
            (commands, visibility) ->
                renderCommands(context, client, commands, visibility, nowMillis, backend)
        );
    }

    static void render(
        Screen currentScreen,
        long nowMillis,
        Supplier<HudFrameInput> frameInputSupplier,
        HudCommandRenderer renderer
    ) {
        Objects.requireNonNull(frameInputSupplier, "frameInputSupplier");
        Objects.requireNonNull(renderer, "renderer");

        ScreenHudVisibility visibility = ScreenHudVisibility.forScreen(currentScreen);
        if (visibility == ScreenHudVisibility.HIDDEN) {
            return;
        }

        HudFrameInput frame = Objects.requireNonNull(frameInputSupplier.get(), "frameInputSupplier.get()");
        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            frame.hudSnapshot(),
            frame.combatSnapshot(),
            nowMillis,
            frame.widthMeasurer(),
            frame.maxTextWidth(),
            frame.screenWidth(),
            frame.screenHeight(),
            frame.botanyAnchor(),
            frame.runtimeContext()
        );
        List<EdgeIndicatorCmd> spiritualSenseIndicators = frame.spiritualSenseIndicators().get();
        List<HudRenderCommand> supplementalCommands = frame.supplementalCommands().get();
        if (!spiritualSenseIndicators.isEmpty() || !supplementalCommands.isEmpty()) {
            commands = new ArrayList<>(commands);
            if (!spiritualSenseIndicators.isEmpty()) {
                PerceptionEdgeRenderer.append(commands, spiritualSenseIndicators);
            }
            if (!supplementalCommands.isEmpty()) {
                commands.addAll(supplementalCommands);
            }
        }

        renderer.render(
            filterCommandsForVisibility(commands, visibility),
            visibility
        );
    }

    private static HudFrameInput captureHudFrameInput(MinecraftClient client, long nowMillis) {
        int screenWidth = client.getWindow().getScaledWidth();
        int screenHeight = client.getWindow().getScaledHeight();
        return new HudFrameInput(
            BongHudStateStore.snapshot(),
            captureCombatSnapshot(client),
            client.textRenderer::getWidth,
            HUD_TEXT_MAX_WIDTH,
            screenWidth,
            screenHeight,
            computeBotanyAnchor(client),
            captureRuntimeContext(client),
            () -> computeSpiritualSenseIndicators(client),
            () -> TiandaoPresenceHudPlanner.buildCommands(
                    TiandaoPresenceStore.snapshot(),
                    nowMillis,
                    screenWidth,
                    screenHeight
                )
        );
    }

    private static void renderCommands(
        DrawContext context,
        MinecraftClient client,
        List<HudRenderCommand> commands,
        ScreenHudVisibility visibility,
        long nowMillis,
        HudRenderBackend backend
    ) {

        drawCommandBatch(context, client, commands, backend);

        renderBaomaiV3HudForProduction(
            new DrawContextHudSurface(context, client),
            nowMillis,
            visibility
        );

        // plan-combat-skill-feedback-bridges-v1 P1 — 爆脉 v4 HUD overlay 接入渲染回路
        // plan-combat-skill-feedback-bridges-v1 P3 — 我流虚蚀视觉 HUD overlay（声音扭曲+阶段文字）
        // plan-fauna-stitched-beast-v1 P3 — 兽核吸收幻觉 HUD overlay（绿边像差+bar偏移+视野旋转）
        if (visibility == ScreenHudVisibility.FULL) {
            CrackReadingOverlay.render(context, client.textRenderer, nowMillis);
            VoidErosionHudOverlay.render(context, client.textRenderer);
            com.bong.client.fauna.HallucinationHudOverlay.render(context);
            long estimatedTick = nowMillis / 50L;
            ResonanceLockMeterHud.render(
                context,
                client.textRenderer,
                client.getWindow().getScaledWidth(),
                client.getWindow().getScaledHeight(),
                estimatedTick,
                nowMillis
            );
        }

        int scaledWidth = client.getWindow().getScaledWidth();
        int scaledHeight = client.getWindow().getScaledHeight();
        for (HudRenderCommand command : commands) {
            if (command.isScreenTint()) {
                OverlayQuadRenderer.render(context, scaledWidth, scaledHeight, command.color());
            } else if (command.isEdgeVignette()) {
                EdgeDecalRenderer.render(context, scaledWidth, scaledHeight, command.color());
            } else if (command.isEdgeInkWash()) {
                InkWashVignetteRenderer.render(context, scaledWidth, scaledHeight, command.color());
            }
        }

        // SVG 提交仍在全屏反馈之后，显式预览时示例不会被 tint/vignette 覆盖。
        backend.render(context, client, visibility);

        // HUD 回调结束时一次性提交所有 GUI layer，避免 SVG 与其他 overlay 交错 flush。
        context.draw();
    }

    /** 生产与截图共用同一命令提交器；不会采样或修改任何 Store。 */
    public static void drawCommandBatch(
        DrawContext context, MinecraftClient client, List<HudRenderCommand> commands, HudRenderBackend backend
    ) {
        for (HudRenderCommand command : commands) {
            if (command.isVector()) {
                backend.renderVector(context, client, command);
                continue;
            }
            if (command.isText()) {
                context.drawTextWithShadow(client.textRenderer, command.text(), command.x(), command.y(), command.color());
                continue;
            }
            if (command.isScaledText()) {
                var matrices = context.getMatrices();
                matrices.push();
                matrices.translate(command.x(), command.y(), 0);
                float scale = (float) command.textScale();
                matrices.scale(scale, scale, 1.0f);
                context.drawTextWithShadow(client.textRenderer, command.text(), 0, 0, command.color());
                matrices.pop();
                continue;
            }
            if (command.isRect()) {
                context.fill(command.x(), command.y(), command.x() + command.width(), command.y() + command.height(), command.color());
                continue;
            }
            if (command.isTexturedRect()) {
                Identifier tex = parseIdentifier(command.texturePath());
                if (tex != null) {
                    context.drawTexture(
                        tex,
                        command.x(), command.y(),
                        0.0f, 0.0f,
                        command.width(), command.height(),
                        command.width(), command.height()
                    );
                }
                continue;
            }
            if (command.isItemTexture()) {
                drawItemTexture(context, command.text(), command.x(), command.y(), command.width());
                continue;
            }
            if (command.isToast()) {
                BongToast.render(
                    context,
                    client.textRenderer,
                    client.getWindow().getScaledWidth(),
                    client.getWindow().getScaledHeight(),
                    command
                );
                continue;
            }
            if (command.isEdgeIndicator()) {
                int size = Math.max(4, (int) Math.round(4.0 + command.intensity() * 6.0));
                context.fill(
                    command.x() - size,
                    command.y() - size,
                    command.x() + size,
                    command.y() + size,
                    command.color()
                );
            }
        }

    }

    @FunctionalInterface
    interface HudCommandRenderer {
        void render(
            List<HudRenderCommand> commands,
            ScreenHudVisibility visibility
        );
    }

    record HudFrameInput(
        com.bong.client.hud.BongHudStateSnapshot hudSnapshot,
        CombatHudSnapshot combatSnapshot,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        BotanyProjection.Anchor botanyAnchor,
        HudRuntimeContext runtimeContext,
        Supplier<List<EdgeIndicatorCmd>> spiritualSenseIndicators,
        Supplier<List<HudRenderCommand>> supplementalCommands
    ) {
        HudFrameInput {
            hudSnapshot = hudSnapshot == null
                ? com.bong.client.hud.BongHudStateSnapshot.empty()
                : hudSnapshot;
            combatSnapshot = combatSnapshot == null ? CombatHudSnapshot.empty() : combatSnapshot;
            widthMeasurer = widthMeasurer == null ? ignored -> 0 : widthMeasurer;
            maxTextWidth = Math.max(0, maxTextWidth);
            screenWidth = Math.max(0, screenWidth);
            screenHeight = Math.max(0, screenHeight);
            runtimeContext = runtimeContext == null ? HudRuntimeContext.empty() : runtimeContext;
            spiritualSenseIndicators = safeListSupplier(spiritualSenseIndicators);
            supplementalCommands = safeListSupplier(supplementalCommands);
        }
    }

    private static <T> Supplier<List<T>> safeListSupplier(Supplier<List<T>> supplier) {
        if (supplier == null) {
            return List::of;
        }
        return () -> {
            List<T> values = supplier.get();
            return values == null ? List.of() : List.copyOf(values);
        };
    }

    static void renderBaomaiV3HudForProduction(
        HudSurface surface,
        long nowMs,
        ScreenHudVisibility visibility
    ) {
        if (visibility == ScreenHudVisibility.FULL) {
            BaomaiV3Hud.render(surface, nowMs);
        }
    }

    private static List<EdgeIndicatorCmd> computeSpiritualSenseIndicators(MinecraftClient client) {
        PerceptionEdgeState state = PerceptionEdgeStateStore.snapshot();
        if (state.isEmpty() || client.gameRenderer == null) {
            return List.of();
        }
        Camera camera = client.gameRenderer.getCamera();
        if (camera == null) {
            return List.of();
        }
        Vec3d camPos = camera.getPos();
        double fov = client.options.getFov().getValue().doubleValue();
        int scaledWidth = client.getWindow().getScaledWidth();
        int scaledHeight = client.getWindow().getScaledHeight();
        List<EdgeIndicatorCmd> indicators = new ArrayList<>();
        for (PerceptionEdgeState.SenseEntry entry : state.entries()) {
            indicators.add(PerceptionEdgeProjector.project(
                entry.x(), entry.y(), entry.z(),
                camPos.x, camPos.y, camPos.z,
                camera.getYaw(), camera.getPitch(),
                fov,
                scaledWidth,
                scaledHeight,
                entry.kind(),
                entry.intensity()
            ));
        }
        return indicators;
    }

    private static Identifier parseIdentifier(String path) {
        if (path == null || path.isBlank()) {
            return null;
        }
        try {
            return new Identifier(path);
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static BotanyProjection.Anchor computeBotanyAnchor(MinecraftClient client) {
        HarvestSessionViewModel session = HarvestSessionStore.snapshot();
        if (!session.hasTargetPos() || client.gameRenderer == null) {
            return null;
        }
        Camera camera = client.gameRenderer.getCamera();
        if (camera == null) {
            return null;
        }
        Vec3d camPos = camera.getPos();
        double[] pos = session.targetPos();
        double fov = client.options.getFov().getValue().doubleValue();
        return BotanyProjection.project(
            pos[0], pos[1], pos[2],
            camPos.x, camPos.y, camPos.z,
            camera.getYaw(), camera.getPitch(),
            fov,
            client.getWindow().getScaledWidth(),
            client.getWindow().getScaledHeight()
        );
    }

    private static CombatHudSnapshot captureCombatSnapshot(MinecraftClient client) {
        return CombatHudSnapshot.create(
            CombatHudStateStore.snapshot(),
            PhysicalBodyStore.snapshot(),
            QuickUseSlotStore.snapshot(),
            SkillBarStore.snapshot(),
            SkillBarStore.selectedSlot(),
            CastStateStore.snapshot(),
            UnifiedEventStore.stream(),
            SpellVolumeStore.snapshot(),
            com.bong.client.combat.store.CarrierStateStore.snapshot(),
            DefenseWindowStore.snapshot(),
            UnlockedStylesStore.snapshot()
        );
    }

    private static HudRuntimeContext captureRuntimeContext(MinecraftClient client) {
        if (client == null || client.getWindow() == null) {
            return HudRuntimeContext.empty();
        }
        Camera camera = client.gameRenderer == null ? null : client.gameRenderer.getCamera();
        double yaw = camera == null ? 0.0 : camera.getYaw();
        double x = 0.0;
        double y = 0.0;
        double z = 0.0;
        if (client.player != null) {
            Vec3d pos = client.player.getPos();
            x = pos.x;
            y = pos.y;
            z = pos.z;
        }
        long handle = client.getWindow().getHandle();
        boolean altDown = InputState.isKeyPressed(handle, GLFW.GLFW_KEY_LEFT_ALT)
            || InputState.isKeyPressed(handle, GLFW.GLFW_KEY_RIGHT_ALT);
        return new HudRuntimeContext(yaw, x, y, z, altDown, List.of());
    }

    private static final class InputState {
        private InputState() {
        }

        private static boolean isKeyPressed(long window, int key) {
            return window != 0L && GLFW.glfwGetKey(window, key) == GLFW.GLFW_PRESS;
        }
    }

    static List<HudRenderCommand> filterCommandsForVisibility(
        List<HudRenderCommand> commands,
        ScreenHudVisibility visibility
    ) {
        Objects.requireNonNull(commands, "commands");
        Objects.requireNonNull(visibility, "visibility");
        return switch (visibility) {
            case FULL -> commands;
            case CAST_BAR_ONLY -> commands.stream()
                .filter(cmd -> cmd.layer() == com.bong.client.hud.HudRenderLayer.CAST_BAR)
                .toList();
            case AGENT_UI_ONLY -> commands.stream()
                .filter(cmd -> cmd.layer() == com.bong.client.hud.HudRenderLayer.AGENT_UI)
                .toList();
            case INVENTORY_DIMMED -> commands.stream()
                .filter(cmd -> {
                    com.bong.client.hud.HudRenderLayer layer = cmd.layer();
                    return layer == com.bong.client.hud.HudRenderLayer.QUICK_BAR
                        || layer == com.bong.client.hud.HudRenderLayer.CAST_BAR
                        || layer == com.bong.client.hud.HudRenderLayer.EVENT_STREAM
                        || layer == com.bong.client.hud.HudRenderLayer.TSY_EXTRACT
                        || layer == com.bong.client.hud.HudRenderLayer.OVERWEIGHT;
                })
                .toList();
            case HIDDEN -> List.of();
        };
    }

    /**
     * Draw a 128×128 source PNG (`bong-client:textures/gui/items/{itemId}.png`)
     * scaled into a {@code size×size} box at {@code (dx, dy)}. Mirrors the
     * approach used in {@code GridSlotComponent.drawItemTexture}.
     */
    private static void drawItemTexture(DrawContext context, String itemId, int dx, int dy, int size) {
        if (itemId == null || itemId.isEmpty() || size <= 0) return;

        // P3 — HUD 快捷栏的 vanilla 方块条目用原生方块图标渲染（与 inspect 屏一致）；
        // 非 vanilla 走扁平贴图。无 BlockItem 的特殊块降级回扁平贴图路径。
        if (com.bong.client.block.BlockVanillaIconMap.drawVanillaIcon(context, itemId, dx, dy, size)) {
            return;
        }

        Identifier tex = GridSlotComponent.textureIdForItemId(itemId);

        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.enableDepthTest();

        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(dx, dy, 100);
        float scale = (float) size / 128.0f;
        matrices.scale(scale, scale, 1.0f);

        context.drawTexture(tex, 0, 0, 128, 128, 0, 0, 128, 128, 128, 128);

        matrices.pop();
        RenderSystem.disableBlend();
    }

    private static final class DrawContextHudSurface implements HudSurface {
        private final DrawContext context;
        private final MinecraftClient client;

        private DrawContextHudSurface(DrawContext context, MinecraftClient client) {
            this.context = context;
            this.client = client;
        }

        @Override
        public int windowWidth() {
            return client.getWindow().getScaledWidth();
        }

        @Override
        public int windowHeight() {
            return client.getWindow().getScaledHeight();
        }

        @Override
        public int measureText(String text) {
            return client.textRenderer.getWidth(text);
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int color) {
            context.fill(x1, y1, x2, y2, color);
        }

        @Override
        public void drawTextWithShadow(String text, int x, int y, int color) {
            context.drawTextWithShadow(client.textRenderer, text, x, y, color);
        }

        @Override
        public void drawText(String text, int x, int y, int color, boolean shadow) {
            context.drawText(client.textRenderer, text, x, y, color, shadow);
        }
    }

    public interface HudSurface {
        int windowWidth();

        int windowHeight();

        int measureText(String text);

        void fill(int x1, int y1, int x2, int y2, int color);

        void drawTextWithShadow(String text, int x, int y, int color);

        void drawText(String text, int x, int y, int color, boolean shadow);
    }

}
