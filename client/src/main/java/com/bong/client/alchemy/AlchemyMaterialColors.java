package com.bong.client.alchemy;

import com.bong.client.inventory.component.GridSlotComponent;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.util.Identifier;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/** 对有效图标像素按透明度加权取平均色；资源重载后重新采样。 */
public final class AlchemyMaterialColors {
    private static final Map<Identifier, Integer> CACHE = new HashMap<>();
    private static final int FALLBACK = 0x9DAF9E;

    private AlchemyMaterialColors() {}

    public static int color(String material) {
        var texture = GridSlotComponent.textureIdForItemId(material);
        return texture == null ? FALLBACK : CACHE.computeIfAbsent(texture, AlchemyMaterialColors::read);
    }

    public static void invalidate() { CACHE.clear(); }

    private static int read(Identifier texture) {
        var resource = MinecraftClient.getInstance().getResourceManager().getResource(texture);
        if (resource.isEmpty()) return FALLBACK;
        try (var stream = resource.get().getInputStream(); var image = NativeImage.read(stream)) {
            long red = 0, green = 0, blue = 0, weight = 0;
            for (int y = 0; y < image.getHeight(); y++) {
                for (int x = 0; x < image.getWidth(); x++) {
                    int pixel = image.getColor(x, y);
                    int alpha = pixel >>> 24;
                    red += (pixel & 255) * alpha;
                    green += ((pixel >>> 8) & 255) * alpha;
                    blue += ((pixel >>> 16) & 255) * alpha;
                    weight += alpha;
                }
            }
            return weight == 0 ? FALLBACK : (int) (red / weight) << 16
                | (int) (green / weight) << 8 | (int) (blue / weight);
        } catch (IOException | IllegalArgumentException exception) {
            return FALLBACK;
        }
    }
}
