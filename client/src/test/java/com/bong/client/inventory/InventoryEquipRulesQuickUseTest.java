package com.bong.client.inventory;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import java.util.Set;
import static org.junit.jupiter.api.Assertions.*;

class InventoryEquipRulesQuickUseTest {
    @AfterEach
    void reset() { QuickUseSlotStore.resetForTests(); }

    @Test
    void eligibilityComesFromServerAndRequiresAnOwnedInstance() {
        InventoryItem pill = InventoryItem.createFull(42L, "guyuan_pill", "固元丹",
            1, 1, 0.2, "rare", "", 1, 1.0, 1.0);
        assertFalse(InventoryEquipRules.canPlaceIntoQuickUse(pill));
        QuickUseSlotStore.replace(QuickSlotConfig.empty().withEligibleItems(Set.of("guyuan_pill")));
        assertTrue(InventoryEquipRules.canPlaceIntoQuickUse(pill));
        assertFalse(InventoryEquipRules.canPlaceIntoQuickUse(InventoryItem.simple("guyuan_pill", "固元丹")));
        assertFalse(InventoryEquipRules.canPlaceIntoQuickUse(InventoryItem.createFull(43L,
            "wooden_shield", "盾", 1, 1, 1.0, "common", "", 1, 1.0, 1.0)));
    }
}
