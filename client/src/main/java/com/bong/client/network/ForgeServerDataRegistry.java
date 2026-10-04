package com.bong.client.network;

import java.util.Map;

/** 炼器 server-data consumer 的注册表；不修改 server forge 生产代码。 */
public final class ForgeServerDataRegistry {
    private ForgeServerDataRegistry() {
    }

    /** 注册炼器工位、会话、结果和蓝图 handler。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        handlers.put("forge_station", new com.bong.client.network.forge.ForgeStationHandler());
        handlers.put("forge_session", new com.bong.client.network.forge.ForgeSessionHandler());
        handlers.put("forge_outcome", new com.bong.client.network.forge.ForgeOutcomeHandler());
        handlers.put(
            "forge_blueprint_book",
            new com.bong.client.network.forge.ForgeBlueprintBookHandler()
        );
    }
}
