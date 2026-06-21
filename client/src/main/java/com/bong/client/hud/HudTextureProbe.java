package com.bong.client.hud;

import net.minecraft.client.MinecraftClient;
import net.minecraft.util.Identifier;

/**
 * HUD 贴图存在性探测：用 MC 资源管理器判断某 Identifier 是否真有对应 png。
 *
 * <p>用作 {@code QuickBarHudPlanner.buildCommands(..., Predicate)} 的注入谓词，让 HUD 在画技能图标前
 * 先确认贴图存在；不存在则回退到文字标签，绝不画紫黑 missing-texture。</p>
 *
 * <p>headless / 单测下 {@code MinecraftClient.getInstance()} 为 null，此时一律返回 false（安全降级）。</p>
 */
public final class HudTextureProbe {
    private HudTextureProbe() {
    }

    public static boolean exists(String path) {
        if (path == null || path.isBlank()) {
            return false;
        }
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.getResourceManager() == null) {
            return false;
        }
        Identifier id = Identifier.tryParse(path);
        if (id == null) {
            return false;
        }
        return client.getResourceManager().getResource(id).isPresent();
    }
}
