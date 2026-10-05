package com.bong.client.network;

import java.util.Map;

/** 需要专门 UI 或反馈 adapter 的小型 server-data consumer 注册表。 */
public final class SpecializedServerDataRegistry {
    private SpecializedServerDataRegistry() {
    }

    /** 注册供应棺、暗器、探针和残卷等专用 handler。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        LootContainerHandler lootContainer = new LootContainerHandler();
        handlers.put("loot_container_open", lootContainer);
        handlers.put("loot_container_update", lootContainer);
        handlers.put("loot_container_close", lootContainer);
        handlers.put("tutorial_coffin_pos", new TutorialCoffinPosHandler());

        com.bong.client.combat.handler.AnqiHudServerDataHandler anqi =
            new com.bong.client.combat.handler.AnqiHudServerDataHandler();
        handlers.put("anqi_hud", anqi);
        com.bong.client.combat.handler.DuguV2ServerDataHandler duguV2 =
            new com.bong.client.combat.handler.DuguV2ServerDataHandler();
        handlers.put("dugu_v2_skill_cast", duguV2);
        handlers.put("dugu_v2_self_cure", duguV2);
        handlers.put("dugu_v2_shroud_active", duguV2);
        handlers.put("permanent_qi_max_decay_applied", duguV2);
        handlers.put(
            "sword_bond_hud_state",
            new com.bong.client.combat.handler.SwordBondHudStateHandler()
        );
        handlers.put(
            "zhenmai_hud",
            new com.bong.client.combat.handler.ZhenmaiHudServerDataHandler()
        );
        handlers.put("mineral_probe_result", new MineralProbeResultHandler());
        handlers.put("inventory_move_rejected", new InventoryMoveRejectedHandler());
        handlers.put("insight_offer", new InsightOfferHandler());
        handlers.put("scroll_open", new ScrollOpenHandler());
    }
}
