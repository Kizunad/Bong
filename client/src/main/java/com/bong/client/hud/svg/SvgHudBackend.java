package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudRenderRegistry;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Optional;
import java.util.EnumSet;
import java.util.Set;

/** SVG HUD 矩形提交后端；示例资源只在显式预览中绘制。 */
public final class SvgHudBackend implements HudRenderBackend {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-svg-hud");
    private static final Identifier EXAMPLE = Identifier.of("bong-client", "svg/hud/example.svg");
    private static final Set<HudRenderLayer> SVG_LAYERS = Set.copyOf(EnumSet.of(
        HudRenderLayer.JIEMAI_RING,
        HudRenderLayer.MOVEMENT_HUD,
        HudRenderLayer.STATUS_EFFECTS
    ));
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
        if (!previewExampleEnabled || context == null || client == null || visibility != ScreenHudVisibility.FULL) {
            return;
        }
        int width = client.getWindow().getScaledWidth();
        int height = client.getWindow().getScaledHeight();
        if (width > 0 && height > 0) {
            renderPreviewExample(context, client, width, height);
        }
    }

    @Override
    public boolean handles(HudRenderCommand command) {
        return command != null
            && SVG_LAYERS.contains(command.layer())
            && command.isRect();
    }

    @Override
    public void renderCommand(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility,
        HudRenderCommand command
    ) {
        if (context == null || client == null || visibility != ScreenHudVisibility.FULL || !handles(command)) {
            return;
        }
        renderCommand(context, client, command);
    }

    private static void renderCommand(DrawContext context, MinecraftClient client, HudRenderCommand command) {
        int screenWidth = client.getWindow().getScaledWidth();
        int screenHeight = client.getWindow().getScaledHeight();
        if (screenWidth <= 0 || screenHeight <= 0) {
            return;
        }
        int x = command.x();
        int y = command.y();
        int width = command.width();
        int height = command.height();
        if (width <= 0 || height <= 0) {
            return;
        }
        HudRenderRegistry.SurfaceDefinition surface = HudRenderRegistry.require(command.layer());
        if (surface.svgAssets().isEmpty()) {
            return;
        }
        Identifier resource = surface.svgAssets().get(0).resource();
        try {
            Optional<SvgMesh> mesh = registry(client.getResourceManager()).find(resource);
            if (mesh.isPresent()) {
                EMITTER.emit(context, mesh.get(), x, y, width, height, command.color());
            }
        } catch (RuntimeException failure) {
            LOGGER.error("[svg] HUD command 提交失败，layer={}", command.layer(), failure);
        }
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

    static void disablePreviewExample() {
        previewExampleEnabled = false;
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
