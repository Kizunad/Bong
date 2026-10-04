package com.bong.client.network;

import java.util.Map;

/** 撤离、容器、移动与灵宝等世界活动 consumer 的注册表。 */
public final class WorldActivityServerDataRegistry {
    private WorldActivityServerDataRegistry() {
    }

    /** 注册世界活动 handler，并保持旧 type set 不变。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        RealmVisionParamsHandler vision = new RealmVisionParamsHandler();
        SpiritualSenseTargetsHandler sense = new SpiritualSenseTargetsHandler();
        ExtractServerDataHandler extract = new ExtractServerDataHandler();
        ContainerInteractionHandler container = new ContainerInteractionHandler();
        com.bong.client.yidao.YidaoServerDataHandler yidao =
            new com.bong.client.yidao.YidaoServerDataHandler();
        MovementStateHandler movement = new MovementStateHandler();
        com.bong.client.spirittreasure.SpiritTreasureStateHandler spiritState =
            new com.bong.client.spirittreasure.SpiritTreasureStateHandler();
        com.bong.client.spirittreasure.SpiritTreasureDialogueHandler spiritDialogue =
            new com.bong.client.spirittreasure.SpiritTreasureDialogueHandler();

        handlers.put("realm_vision_params", vision);
        handlers.put("spiritual_sense_targets", sense);
        handlers.put("rift_portal_state", extract);
        handlers.put("rift_portal_removed", extract);
        handlers.put("extract_started", extract);
        handlers.put("extract_progress", extract);
        handlers.put("extract_completed", extract);
        handlers.put("extract_aborted", extract);
        handlers.put("extract_failed", extract);
        handlers.put("tsy_collapse_started_ipc", extract);
        handlers.put("container_state", container);
        handlers.put("search_started", container);
        handlers.put("search_progress", container);
        handlers.put("search_completed", container);
        handlers.put("search_aborted", container);
        handlers.put("healer_npc_ai_state", yidao);
        handlers.put("yidao_hud_state", yidao);
        handlers.put("movement_state", movement);
        handlers.put("spirit_treasure_state", spiritState);
        handlers.put("spirit_treasure_dialogue", spiritDialogue);
        handlers.put("coffin_state", new CoffinStateHandler());
    }
}
