package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Optional;

/** SVG GUI 提交基础设施；当前仅在显式测试预览中绘制示例。 */
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
        // 预览示例必须由显式环境变量打开；正常联机环境保持 fail closed。
        if ("1".equals(System.getenv("BONG_SVG_HUD_PREVIEW"))) {
            enablePreviewExample();
        }
        return INSTANCE;
    }

    @Override
    public void render(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility
    ) {
        // 没有生产 SVG surface 时直接退出，正常游戏不读取示例资源或构造 mesh。
        if (!previewExampleEnabled || context == null || client == null || visibility != ScreenHudVisibility.FULL) {
            return;
        }
        int width = client.getWindow().getScaledWidth();
        int height = client.getWindow().getScaledHeight();
        if (width <= 0 || height <= 0) {
            return;
        }
        renderPreviewExample(context, client, width, height);
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
