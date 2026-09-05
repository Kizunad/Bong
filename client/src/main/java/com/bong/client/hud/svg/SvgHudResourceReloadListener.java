package com.bong.client.hud.svg;

import net.fabricmc.fabric.api.resource.ResourceManagerHelper;
import net.fabricmc.fabric.api.resource.SimpleSynchronousResourceReloadListener;
import net.minecraft.resource.ResourceManager;
import net.minecraft.resource.ResourceType;
import net.minecraft.util.Identifier;

/** 资源重载阶段重建 SVG HUD 的成功与失败缓存，避免解析进入正常渲染帧。 */
public final class SvgHudResourceReloadListener implements SimpleSynchronousResourceReloadListener {
    private static final Identifier ID = Identifier.of("bong-client", "svg_hud");

    public static void register() {
        ResourceManagerHelper.get(ResourceType.CLIENT_RESOURCES)
            .registerReloadListener(new SvgHudResourceReloadListener());
    }

    @Override
    public Identifier getFabricId() {
        return ID;
    }

    @Override
    public void reload(ResourceManager manager) {
        SvgHudBackend.invalidateAssets();
        SvgHudBackend.registry(manager);
    }
}
