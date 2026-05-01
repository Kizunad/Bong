package com.bong.client.alchemy;

import java.util.Set;

public final class AlchemyFurnaceItems {
    public static final Set<String> FURNACE_ITEMS = Set.of(
        "furnace_fantie",
        "furnace_lingtie",
        "furnace_xitie"
    );

    private AlchemyFurnaceItems() {}

    public static boolean isFurnaceItem(String itemId) {
        return FURNACE_ITEMS.contains(itemId);
    }
}
