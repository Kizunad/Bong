package com.bong.client.combat;

import com.bong.client.combat.inspect.WeaponTreasurePanel;

import java.util.ArrayList;
import java.util.List;

public final class TreasurePanelSync {
    private static final String[] TREASURE_SLOTS = {
        "off_hand",
        "treasure_belt_0",
        "treasure_belt_1",
        "treasure_belt_2",
        "treasure_belt_3"
    };

    private TreasurePanelSync() {
    }

    public static void syncFromStore() {
        List<WeaponTreasurePanel.Treasure> treasures = new ArrayList<>();
        for (String slot : TREASURE_SLOTS) {
            EquippedTreasure treasure = TreasureEquippedStore.get(slot);
            if (treasure == null) continue;
            treasures.add(new WeaponTreasurePanel.Treasure(
                treasure.templateId(),
                treasure.displayName(),
                slot.equals("off_hand") ? "副手" : "腰带",
                1.0f,
                1.0f,
                List.of(),
                List.of()
            ));
        }
        WeaponTreasurePanel.replaceTreasures(treasures);
    }
}
