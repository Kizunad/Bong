package com.bong.client.util;

import net.minecraft.client.MinecraftClient;
import net.minecraft.util.Identifier;

/**
 * 资源贴图存在性探测：用 MC 资源管理器判断某 Identifier 是否真有对应 png。
 *
 * <p>作为贴图存在性谓词注入各渲染规划器，让调用方在绘制图标前先确认贴图存在；
 * 不存在则由调用方选择安全的回退表现，绝不画紫黑 missing-texture。</p>
 *
 * <p>headless / 单测下 {@code MinecraftClient.getInstance()} 为 null，此时一律返回 false（安全降级）。</p>
 */
public final class TextureProbe {
    private TextureProbe() {
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
