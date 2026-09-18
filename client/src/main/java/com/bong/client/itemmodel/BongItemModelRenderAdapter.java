package com.bong.client.itemmodel;

import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.item.ItemRenderer;
import net.minecraft.client.render.model.json.ModelTransformationMode;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.item.ItemStack;

/**
 * Direct-render adapter for the P0 model channel.
 *
 * <p>Minecraft's public baked-model render overload still carries an
 * {@link ItemStack} parameter for glint and dynamic-display decisions. The
 * adapter passes {@link ItemStack#EMPTY}; the model and all transforms come
 * from {@link BongItemModelChannel.ModelHandle}. It never creates, registers,
 * or maps a fake vanilla item stack.</p>
 */
public final class BongItemModelRenderAdapter {
    private BongItemModelRenderAdapter() {
    }

    /**
     * Renders a Bong model directly when the caller is already in an item-render
     * context. Returns false for unknown, incomplete, or missing definitions.
     */
    public static boolean render(
        ItemRenderer itemRenderer,
        String templateId,
        ModelTransformationMode transformationMode,
        boolean leftHanded,
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light,
        int overlay
    ) {
        return BongItemModelChannel.lookup(templateId).map(handle -> {
            itemRenderer.renderItem(
                ItemStack.EMPTY,
                transformationMode,
                leftHanded,
                matrices,
                vertexConsumers,
                light,
                overlay,
                handle.model()
            );
            return true;
        }).orElse(false);
    }
}
