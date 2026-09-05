package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderRegistry;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Optional;

/** 生产 HUD 的缓存矢量后端；测试示例仍仅在显式预览开关下加载。 */
public final class SvgHudBackend implements HudRenderBackend {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-svg-hud");
    private static final Identifier EXAMPLE = Identifier.of("bong-client", "svg/hud/example.svg");
    private static final MinecraftGuiMeshEmitter EMITTER = new MinecraftGuiMeshEmitter();
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
    public void renderVector(DrawContext context, MinecraftClient client, HudRenderCommand command) {
        if (context == null || client == null || !command.isVector()
            || command.width() <= 0 || command.height() <= 0 || (command.color() >>> 24) == 0) {
            return;
        }
        Identifier resource = HudRenderRegistry.requireVectorAsset(command.layer(), command.text());
        // 成功和失败结果都缓存；每帧仅改变几何变换与颜色，不再构造 SVG 字符串。
        registry(client.getResourceManager()).find(resource).ifPresent(mesh ->
            EMITTER.emitFitted(context, mesh, command.x(), command.y(),
                command.width(), command.height(), command.color()));
    }

    @Override
    public void render(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility
    ) {
        // 测试示例独立门控；生产 HUD 由原命令位置的 renderVector 提交。
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
        // 资源代际由 reload listener 切换；客户端外层 manager 与回调内层 manager 不一定同一对象。
        if (current == null) {
            synchronized (SvgHudBackend.class) {
                current = registry;
                if (current == null) {
                    current = new SvgHudAssetRegistry(resourceManager);
                    // 首次资源加载及 F3+T 阶段预热生产白名单；渲染帧只取缓存，示例不在此列。
                    SvgHudAssetRegistry prepared = current;
                    HudRenderRegistry.productionSurfaces().stream()
                        .flatMap(surface -> surface.svgAssets().stream())
                        .map(HudRenderRegistry.SvgAsset::resource).distinct()
                        .forEach(prepared::find);
                    registry = current;
                }
            }
        }
        return current;
    }

    /** 丢弃成功和失败缓存；重载监听器随即使用新资源包预热生产资源。 */
    static void invalidateAssets() {
        synchronized (SvgHudBackend.class) {
            if (registry != null) {
                registry.clear();
            }
            registry = null;
        }
    }

    static void resetForTests() {
        invalidateAssets();
        previewExampleEnabled = false;
    }
}
