package com.bong.client.network;

import java.util.Map;

/** 炼丹领域 server-data handler 的注册表；不负责切换炼丹生产路径。 */
public final class AlchemyServerDataRegistry {
    private AlchemyServerDataRegistry() {
    }

    /** 注册炼丹状态和结果 consumer。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        com.bong.client.network.alchemy.AlchemyFurnaceHandler furnace =
            new com.bong.client.network.alchemy.AlchemyFurnaceHandler();
        com.bong.client.network.alchemy.AlchemySessionHandler session =
            new com.bong.client.network.alchemy.AlchemySessionHandler();
        com.bong.client.network.alchemy.AlchemyOutcomeForecastHandler forecast =
            new com.bong.client.network.alchemy.AlchemyOutcomeForecastHandler();
        com.bong.client.network.alchemy.AlchemyRecipeBookHandler recipeBook =
            new com.bong.client.network.alchemy.AlchemyRecipeBookHandler();
        com.bong.client.network.alchemy.AlchemyContaminationHandler contamination =
            new com.bong.client.network.alchemy.AlchemyContaminationHandler();
        com.bong.client.network.alchemy.AlchemyOutcomeResolvedHandler resolved =
            new com.bong.client.network.alchemy.AlchemyOutcomeResolvedHandler();

        handlers.put("alchemy_furnace", furnace);
        handlers.put("alchemy_world", new com.bong.client.network.alchemy.AlchemyWorldHandler());
        handlers.put("alchemy_session", session);
        handlers.put("alchemy_outcome_forecast", forecast);
        handlers.put("alchemy_recipe_book", recipeBook);
        handlers.put("alchemy_contamination", contamination);
        handlers.put("alchemy_outcome_resolved", resolved);
    }
}
