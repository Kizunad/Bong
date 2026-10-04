package com.bong.client.render;

import com.bong.client.inventory.ItemIconRegistry;
import net.minecraft.util.Identifier;

/** 领域无关的物品贴图解析入口，避免业务 UI 反向依赖库存组件。 */
public final class ItemTextureResolver {
    private ItemTextureResolver() {}

    public static Identifier forItemId(String itemId) {
        return ItemIconRegistry.textureIdForItemId(itemId);
    }
}
