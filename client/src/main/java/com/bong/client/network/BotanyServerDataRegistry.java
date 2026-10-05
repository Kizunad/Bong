package com.bong.client.network;

import java.util.Map;

/** 植物、采集和加工进度 consumer 的分域注册表。 */
public final class BotanyServerDataRegistry {
    private BotanyServerDataRegistry() {
    }

    /** 注册 botany/gathering/processing handler。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        handlers.put("botany_harvest_progress", new BotanyHarvestProgressHandler());
        handlers.put("gathering_session", new GatheringSessionHandler());
        handlers.put("mining_progress", new MiningProgressHandler());
        handlers.put("lumber_progress", new LumberProgressHandler());
        handlers.put("botany_plant_v2_render_profiles", new BotanyPlantRenderProfileHandler());
        handlers.put("botany_skill", new BotanySkillHandler());

        com.bong.client.network.processing.ProcessingServerDataHandler processing =
            new com.bong.client.network.processing.ProcessingServerDataHandler();
        handlers.put("processing_session", processing);
        handlers.put("freshness_update", processing);
    }
}
