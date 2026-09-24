package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryModel;

import java.util.Objects;

/** 搜刮屏不可变投影；Screen 不再直接读取 loot/inventory Store。 */
public record LootContainerScreenViewModel(
    long revision,
    LootContainerSession session,
    InventoryModel inventory
) {
    public LootContainerScreenViewModel {
        if (revision < 0L) throw new IllegalArgumentException("revision must be >= 0");
        Objects.requireNonNull(session, "session must not be null");
        Objects.requireNonNull(inventory, "inventory must not be null");
    }

    public LootContainerSession.Open openSession() {
        return session instanceof LootContainerSession.Open open ? open : null;
    }
}
