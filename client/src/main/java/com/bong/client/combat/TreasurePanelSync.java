package com.bong.client.combat;

import com.bong.client.combat.inspect.WeaponTreasurePanel;

import java.util.ArrayList;
import java.util.List;

public final class TreasurePanelSync {
    // plan-layered-equip-v1 P4（决议 #8）：treasure_belt_0..3 槽已删（PR-1）。法宝激活态迁灵宝 UI 触发位
    // （后续接入：触发位 store / payload，见 plan §P4「法宝激活触发位」TODO）。当前仅同步 off_hand 持械法宝。
    private static final String[] TREASURE_SLOTS = {
        "off_hand"
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
                "副手",
                1.0f,
                1.0f,
                List.of(),
                List.of()
            ));
        }
        WeaponTreasurePanel.replaceTreasures(treasures);
    }
}
