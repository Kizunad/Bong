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

/** SVG HUD 资产与矩形提交后端。 */
public final class SvgHudBackend implements HudRenderBackend {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-svg-hud");
    private static final Set<HudRenderLayer> SVG_LAYERS = Set.copyOf(EnumSet.of(
        HudRenderLayer.JIEMAI_RING,
        HudRenderLayer.MOVEMENT_HUD,
        HudRenderLayer.STATUS_EFFECTS
    ));
    private static final MinecraftGuiMeshEmitter EMITTER = new MinecraftGuiMeshEmitter();
    private static volatile ResourceManager lastResourceManager;
    private static volatile SvgHudAssetRegistry registry;

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
        ScreenHudVisibility visibility
    ) {
        // 所有几何均由 renderCommand 按 HUD 命令顺序提交。
    }

    @Override
    public boolean handles(HudRenderCommand command) {
        return command != null
            && (command.isSvgRect() || (SVG_LAYERS.contains(command.layer()) && command.isRect()));
    }

    @Override
    public void renderCommand(
        DrawContext context,
        MinecraftClient client,
        ScreenHudVisibility visibility,
        HudRenderCommand command
    ) {
        if (context == null || client == null || !handles(command) || !visible(command.layer(), visibility)) {
            return;
        }
        renderCommand(context, client, command);
    }

    static boolean visible(HudRenderLayer layer, ScreenHudVisibility visibility) {
        return switch (visibility) {
            case FULL -> true;
            case CAST_BAR_ONLY -> layer == HudRenderLayer.CAST_BAR;
            case INVENTORY_DIMMED -> layer == HudRenderLayer.QUICK_BAR || layer == HudRenderLayer.CAST_BAR;
            case HIDDEN, AGENT_UI_ONLY -> false;
        };
    }

    static Optional<Identifier> resourceFor(HudRenderCommand command) {
        String key = command.isSvgRect() ? command.svgAssetKey() : "rect";
        return HudRenderRegistry.require(command.layer()).svgAssets().stream()
            .filter(asset -> asset.key().equals(key))
            .map(HudRenderRegistry.SvgAsset::resource)
            .findFirst();
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
        Optional<Identifier> resource = resourceFor(command);
        if (resource.isEmpty()) {
            return;
        }
        try {
            Optional<SvgMesh> mesh = registry(client.getResourceManager()).find(resource.get());
            if (mesh.isPresent()) {
                SvgMesh fitted = mesh.get();
                EMITTER.emit(context, fitted, x, y, width / fitted.width(), height / fitted.height(), command.color());
            }
        } catch (RuntimeException failure) {
            LOGGER.error("[svg] HUD command 提交失败，layer={}", command.layer(), failure);
        }
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
    }
}
