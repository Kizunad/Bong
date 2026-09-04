package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.HudRenderRegistry;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.SvgHudFrame;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Optional;

/** R7 P4 首个真实 SVG layer：QI_RADAR。 */
public final class SvgHudBackend implements HudRenderBackend {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-svg-hud");
    private static final Identifier EXAMPLE = Identifier.of("bong-client", "svg/hud/example.svg");
    private static final MinecraftGuiMeshEmitter EMITTER = new MinecraftGuiMeshEmitter();
    private static volatile ResourceManager lastResourceManager;
    private static volatile SvgHudAssetRegistry registry;
    private static volatile boolean previewExampleEnabled;

    private SvgHudBackend() {
    }

    private static final SvgHudBackend INSTANCE = new SvgHudBackend();

    /** 由客户端组合根注入 HUD 回调，表现层外不暴露具体实现细节。 */
    public static HudRenderBackend production() {
        return INSTANCE;
    }

    @Override
    public void render(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility,
        SvgHudFrame frame
    ) {
        renderLayers(context, client, visibility, frame);
    }

    private void renderLayers(
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
            renderPreviewExample(context, client, width, height);
        }
        SvgHudFrame safeFrame = frame == null ? SvgHudFrame.hidden() : frame;
        if (!safeFrame.visible()) {
            return;
        }
        // 生产 layer 只消费应用层已经规划好的 frame，不在这里反查业务 Store。
        emit(
            context,
            client,
            HudRenderRegistry.requireSvgAsset(HudRenderLayer.QI_RADAR, "radar"),
            safeFrame.x(),
            safeFrame.y(),
            safeFrame.scale(),
            safeFrame.tint()
        );
    }

    /** 预览专用示例，放在右上方以避开左下角既有 HUD。 */
    private static void renderPreviewExample(DrawContext context, MinecraftClient client, int width, int height) {
        int panelWidth = 180;
        int panelHeight = 72;
        int x = Math.max(8, width - panelWidth - 12);
        int y = Math.max(40, Math.min(48, height - panelHeight - 8));
        emit(context, client, EXAMPLE, x, y, 1.0f, 0xFFFFFFFF);
    }

    private static void emit(
        DrawContext context,
        MinecraftClient client,
        Identifier resource,
        int x,
        int y,
        float scale,
        int tint
    ) {
        Optional<SvgMesh> mesh = registry(client.getResourceManager()).find(resource);
        if (mesh.isEmpty()) {
            return;
        }
        try {
            EMITTER.emit(context, mesh.get(), x, y, scale, tint);
        } catch (RuntimeException failure) {
            // 几何提交失败不影响其余 HUD；资源失败已由 registry 缓存并仅记录一次。
            LOGGER.error("[svg] HUD mesh 提交失败，资源={}", resource, failure);
        }
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

    /** F3+T 完成后失效成功和失败缓存，让下次帧从新资源包重新加载。 */
    static void invalidateAssets() {
        synchronized (SvgHudBackend.class) {
            if (registry != null) {
                registry.clear();
            }
            registry = null;
            lastResourceManager = null;
        }
    }

    static void resetForTests() {
        invalidateAssets();
        previewExampleEnabled = false;
    }
}
