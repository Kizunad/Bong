package com.bong.client.mixin;

import com.bong.client.alchemy.AlchemyFurnaceInteractionRules;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestProtocol;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MixinClientPlayerInteractionManagerAlchemyTest {

    @AfterEach
    void tearDown() {
        SkillBarStore.resetForTests();
    }

    @Test
    void onlyKnownFurnacePositionOpensAlchemyUi() {
        BlockPos known = new BlockPos(-12, 64, 38);
        AlchemyFurnaceStore.Snapshot snapshot = new AlchemyFurnaceStore.Snapshot(
            known, 1, 92f, 100f, "self", false
        );

        assertTrue(AlchemyFurnaceInteractionRules.shouldOpenAlchemyFurnace(known, snapshot));
        assertFalse(AlchemyFurnaceInteractionRules.shouldOpenAlchemyFurnace(new BlockPos(-12, 64, 39), snapshot));
        assertFalse(AlchemyFurnaceInteractionRules.shouldOpenAlchemyFurnace(known, AlchemyFurnaceStore.Snapshot.empty()));
    }

    @Test
    void selectedBlockItemPrefersMatchingSelectedHotbarInstance() {
        InventoryItem selected = InventoryItem.createFull(
            42L, "earth_crumb", "土块", 1, 1, 0.2, "common", "", 1, 1.0, 1.0
        );
        InventoryItem fallback = InventoryItem.createFull(
            43L, "earth_crumb", "土块", 1, 1, 0.2, "common", "", 1, 1.0, 1.0
        );
        InventoryModel inventory = InventoryModel.builder()
            .hotbar(0, fallback)
            .hotbar(2, selected)
            .build();

        SkillBarStore.updateSlot(2, SkillBarEntry.item("earth_crumb", "土块", 0, 0, ""));

        assertSame(selected, MixinClientPlayerInteractionManagerAlchemy.bong$selectedBlockItem(2, inventory));
        assertEquals(42L, MixinClientPlayerInteractionManagerAlchemy.bong$selectedBlockItem(2, inventory).instanceId());
    }

    @Test
    void selectedBlockItemRejectsUnknownOrMissingInstances() {
        InventoryModel inventory = InventoryModel.builder()
            .gridItem(InventoryItem.createFull(
                0L, "earth_crumb", "土块", 1, 1, 0.2, "common", "", 1, 1.0, 1.0
            ), 0, 0)
            .build();

        SkillBarStore.updateSlot(0, SkillBarEntry.item("earth_crumb", "土块", 0, 0, ""));
        assertNull(MixinClientPlayerInteractionManagerAlchemy.bong$selectedBlockItem(0, inventory));

        SkillBarStore.updateSlot(0, SkillBarEntry.item("unknown_block", "未知", 0, 0, ""));
        assertNull(MixinClientPlayerInteractionManagerAlchemy.bong$selectedBlockItem(0, inventory));
    }

    @Test
    void selectedBlockRightClickBuildsBlockPlaceIntent() {
        InventoryItem selected = InventoryItem.createFull(
            72L, "earth_crumb", "土块", 1, 1, 0.2, "common", "", 1, 1.0, 1.0
        );
        InventoryItem fallback = InventoryItem.createFull(
            73L, "earth_crumb", "土块", 1, 1, 0.2, "common", "", 1, 1.0, 1.0
        );
        InventoryModel inventory = InventoryModel.builder()
            .hotbar(0, fallback)
            .hotbar(3, selected)
            .build();
        BlockPos hitPos = new BlockPos(10, 64, -3);

        SkillBarStore.updateSlot(3, SkillBarEntry.item("earth_crumb", "土块", 0, 0, ""));

        MixinClientPlayerInteractionManagerAlchemy.BongBlockPlaceIntent intent =
            MixinClientPlayerInteractionManagerAlchemy.bong$selectedBlockPlaceIntent(
                3, inventory, hitPos, Direction.EAST
            );

        assertNotNull(intent);
        assertEquals(hitPos.offset(Direction.EAST), intent.placePos());
        assertEquals(72L, intent.instanceId());
        assertEquals(ClientRequestProtocol.ZhenfaTargetFace.EAST, intent.face());
    }

    @Test
    void selectedBlockPlaceIntentRejectsUnknownOrMissingInstances() {
        InventoryModel missingInstanceInventory = InventoryModel.builder()
            .hotbar(0, InventoryItem.createFull(
                0L, "earth_crumb", "土块", 1, 1, 0.2, "common", "", 1, 1.0, 1.0
            ))
            .build();
        InventoryModel unknownInventory = InventoryModel.builder()
            .hotbar(0, InventoryItem.createFull(
                81L, "unknown_block", "未知", 1, 1, 0.2, "common", "", 1, 1.0, 1.0
            ))
            .build();
        BlockPos hitPos = new BlockPos(10, 64, -3);

        SkillBarStore.updateSlot(0, SkillBarEntry.item("earth_crumb", "土块", 0, 0, ""));
        assertNull(MixinClientPlayerInteractionManagerAlchemy.bong$selectedBlockPlaceIntent(
            0, missingInstanceInventory, hitPos, Direction.UP
        ));

        SkillBarStore.updateSlot(0, SkillBarEntry.item("unknown_block", "未知", 0, 0, ""));
        assertNull(MixinClientPlayerInteractionManagerAlchemy.bong$selectedBlockPlaceIntent(
            0, unknownInventory, hitPos, Direction.UP
        ));
    }
}
