package com.bong.client.combat;

import com.bong.client.combat.inspect.WeaponTreasurePanel;

import java.util.ArrayList;
import java.util.List;

public final class TreasurePanelSync {
    // plan-layered-equip-v1 P4（决议 #8）：treasure_belt_0..3 槽已删（PR-1）。法宝激活态迁灵宝 UI 触发位。
    // 战斗 HUD 法宝面板从触发位（trigger_0..trigger_3）拉激活态法宝 + off_hand 持械法宝展示。
    // 触发位 slot 命名须与 server treasure_equipped_emit.rs `trigger_slot_key` 对齐。
    public static final int TREASURE_TRIGGER_CAP = 4;

    private static final String[] TREASURE_SLOTS = buildSlots();

    private static String[] buildSlots() {
        String[] slots = new String[TREASURE_TRIGGER_CAP + 1];
        slots[0] = "off_hand";
        for (int i = 0; i < TREASURE_TRIGGER_CAP; i++) {
            slots[i + 1] = triggerSlotKey(i);
        }
        return slots;
    }

    public static String triggerSlotKey(int index) {
        return "trigger_" + index;
    }

    private TreasurePanelSync() {
    }

    public static void syncFromStore() {
        List<WeaponTreasurePanel.Treasure> treasures = new ArrayList<>();
        for (String slot : TREASURE_SLOTS) {
            EquippedTreasure treasure = TreasureEquippedStore.get(slot);
            if (treasure == null) continue;
            boolean isTrigger = slot.startsWith("trigger_");
            treasures.add(new WeaponTreasurePanel.Treasure(
                treasure.templateId(),
                treasure.displayName(),
                isTrigger ? "触发位" : "副手",
                1.0f,
                1.0f,
                List.of(),
                List.of()
            ));
        }
        WeaponTreasurePanel.replaceTreasures(treasures);
    }
}
