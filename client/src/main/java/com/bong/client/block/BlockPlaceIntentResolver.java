package com.bong.client.block;

import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestProtocol;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;

public final class BlockPlaceIntentResolver {
    private BlockPlaceIntentResolver() {
    }

    public static InventoryItem selectedBlockItem(int selectedSlot, InventoryModel inventory) {
        if (selectedSlot < 0 || selectedSlot >= SkillBarConfig.SLOT_COUNT || inventory == null) return null;

        SkillBarEntry entry = SkillBarStore.snapshot().slot(selectedSlot);
        if (entry == null || entry.kind() != SkillBarEntry.Kind.ITEM) return null;
        if (!BlockVanillaIconMap.isKnownBlockItem(entry.id())) return null;

        InventoryItem hotbarItem = selectedSlot < inventory.hotbar().size()
            ? inventory.hotbar().get(selectedSlot)
            : null;
        if (matchesInstance(hotbarItem, entry.id())) return hotbarItem;

        for (InventoryItem item : inventory.hotbar()) {
            if (matchesInstance(item, entry.id())) return item;
        }
        for (var gridEntry : inventory.gridItems()) {
            InventoryItem item = gridEntry.item();
            if (matchesInstance(item, entry.id())) return item;
        }
        return null;
    }

    public static Intent selectedBlockPlaceIntent(
        int selectedSlot,
        InventoryModel inventory,
        BlockPos targetPos,
        Direction targetFace
    ) {
        if (targetPos == null || targetFace == null) return null;
        InventoryItem selectedBlock = selectedBlockItem(selectedSlot, inventory);
        if (selectedBlock == null || selectedBlock.instanceId() <= 0) return null;
        return new Intent(
            targetPos.offset(targetFace),
            selectedBlock.instanceId(),
            zhenfaFace(targetFace)
        );
    }

    public static ClientRequestProtocol.ZhenfaTargetFace zhenfaFace(Direction direction) {
        return switch (direction) {
            case UP -> ClientRequestProtocol.ZhenfaTargetFace.TOP;
            case DOWN -> ClientRequestProtocol.ZhenfaTargetFace.BOTTOM;
            case NORTH -> ClientRequestProtocol.ZhenfaTargetFace.NORTH;
            case SOUTH -> ClientRequestProtocol.ZhenfaTargetFace.SOUTH;
            case EAST -> ClientRequestProtocol.ZhenfaTargetFace.EAST;
            case WEST -> ClientRequestProtocol.ZhenfaTargetFace.WEST;
        };
    }

    public record Intent(
        BlockPos placePos,
        long instanceId,
        ClientRequestProtocol.ZhenfaTargetFace face
    ) {
    }

    private static boolean matchesInstance(InventoryItem item, String templateId) {
        return item != null
            && !item.isEmpty()
            && item.instanceId() > 0
            && templateId.equals(item.itemId());
    }
}
