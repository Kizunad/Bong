package com.bong.client.network;

import java.util.Map;
import java.util.Objects;

/** 通用 craft consumer 的 contract 注册表；不启用 craft production traffic。 */
public final class CraftServerDataRegistry {
    private static final ServerDataHandler UNCONFIGURED_WORKBENCH_HANDLER = envelope ->
        ServerDataDispatch.noOp(envelope.type(), "workbench_open handler is not installed");
    private static volatile ServerDataHandler workbenchOpenHandler = UNCONFIGURED_WORKBENCH_HANDLER;

    private CraftServerDataRegistry() {
    }

    /**
     * 安装由 craft bootstrap 提供的工作台入口。
     *
     * <p>网络注册表只持有 {@link ServerDataHandler} 抽象，不反向依赖窗口或
     * Minecraft UI。客户端启动器必须在创建默认路由前调用此方法。</p>
     */
    public static void installWorkbenchOpenHandler(ServerDataHandler handler) {
        workbenchOpenHandler = Objects.requireNonNull(handler, "handler");
    }

    /** 注册既有通用制作 handler。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        register(handlers, workbenchOpenHandler);
    }

    /**
     * 以调用方提供的抽象 handler 注册 contract，供路由装配和契约测试使用。
     */
    static void register(Map<String, ServerDataHandler> handlers, ServerDataHandler workbenchHandler) {
        handlers.put("craft_recipe_list", new CraftRecipeListHandler());
        handlers.put("craft_session_state", new CraftSessionStateHandler());
        handlers.put("craft_outcome", new CraftOutcomeHandler());
        handlers.put("recipe_unlocked", new RecipeUnlockedHandler());
        handlers.put("workbench_open", Objects.requireNonNull(workbenchHandler, "workbenchHandler"));
    }
}
