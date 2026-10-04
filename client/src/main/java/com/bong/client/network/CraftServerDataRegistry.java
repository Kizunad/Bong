package com.bong.client.network;

import java.util.Map;

/** 通用 craft consumer 的 contract 注册表；不启用 craft production traffic。 */
public final class CraftServerDataRegistry {
    private CraftServerDataRegistry() {
    }

    /** 注册既有通用制作 handler。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        handlers.put("craft_recipe_list", new CraftRecipeListHandler());
        handlers.put("craft_session_state", new CraftSessionStateHandler());
        handlers.put("craft_outcome", new CraftOutcomeHandler());
        handlers.put("recipe_unlocked", new RecipeUnlockedHandler());
        handlers.put("workbench_open", com.bong.client.craft.WorkbenchScreenBootstrap.handler());
    }
}
