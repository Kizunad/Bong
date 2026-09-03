package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.SvgHudFrame;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/** R7 P4 首个真实 SVG layer：QI_RADAR。 */
public final class SvgHudBackend implements HudRenderBackend {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-svg-hud");
    private static final Identifier QI_RADAR = Identifier.of("bong-client", "svg/hud/qi-radar.svg");
    private static final Identifier EXAMPLE = Identifier.of("bong-client", "svg/hud/example.svg");
    private static final MinecraftGuiMeshEmitter EMITTER = new MinecraftGuiMeshEmitter();
    private static volatile ResourceManager lastResourceManager;
    private static volatile SvgHudAssetRegistry registry;
    private static volatile boolean previewExampleEnabled;

    private SvgHudBackend() {
    }

    public static void renderProduction(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility,
        SvgHudFrame frame
    ) {
        INSTANCE.renderLayer(context, client, visibility, frame);
    }

    private static final SvgHudBackend INSTANCE = new SvgHudBackend();

    @Override
    public void render(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility,
        SvgHudFrame frame
    ) {
        renderLayer(context, client, visibility, frame);
    }

    private void renderLayer(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility,
        SvgHudFrame frame
    ) {
        if (context == null || client == null || visibility != ScreenHudVisibility.FULL) {
            return;
        }
        int width = client.getWindow().getScaledWidth();
        int height = client.getWindow().getScaledHeight();
        if (width <= 0 || height <= 0) {
            return;
        }
        // 示例是独立的预览 fixture，不应被服务器 player_state 覆盖本地境界而隐藏。
        if (previewExampleEnabled) {
            try {
                renderPreviewExample(context, client, width, height);
            } catch (RuntimeException failure) {
                LOGGER.error("[svg] example.svg 资源加载/提交失败，已跳过本帧", failure);
            }
        }
        SvgHudFrame safeFrame = frame == null ? SvgHudFrame.hidden() : frame;
        if (!safeFrame.visible()) {
            return;
        }
        // 生产 layer 只消费应用层已经规划好的 frame，不在这里反查业务 Store。
        try {
            SvgMesh mesh = registry(client.getResourceManager()).get(QI_RADAR);
            EMITTER.emit(
                context,
                mesh,
                safeFrame.x(),
                safeFrame.y(),
                safeFrame.scale(),
                safeFrame.tint()
            );
        } catch (RuntimeException failure) {
            // 资源错误必须 fail closed，不能让 HUD 线程因单个 SVG 崩溃。
            LOGGER.error("[svg] QI_RADAR 资源加载/提交失败，已跳过本帧", failure);
        }
    }

    /** 预览专用示例，放在右上方以避开左下角既有 HUD。 */
    private static void renderPreviewExample(DrawContext context, MinecraftClient client, int width, int height) {
        SvgMesh mesh = registry(client.getResourceManager()).get(EXAMPLE);
        int panelWidth = 180;
        int panelHeight = 72;
        int x = Math.max(8, width - panelWidth - 12);
        int y = Math.max(40, Math.min(48, height - panelHeight - 8));
        EMITTER.emit(context, mesh, x, y, 1.0f, 0xFFFFFFFF);
    }

    static void enablePreviewExample() {
        previewExampleEnabled = true;
    }

    static SvgHudAssetRegistry registry(ResourceManager resourceManager) {
        SvgHudAssetRegistry current = registry;
        if (current == null || lastResourceManager != resourceManager) {
            synchronized (SvgHudBackend.class) {
                current = registry;
                if (current == null || lastResourceManager != resourceManager) {
                    current = new SvgHudAssetRegistry(resourceManager);
                    registry = current;
                    lastResourceManager = resourceManager;
                }
            }
        }
        return current;
    }

    static void resetForTests() {
        registry = null;
        lastResourceManager = null;
        previewExampleEnabled = false;
    }
}
