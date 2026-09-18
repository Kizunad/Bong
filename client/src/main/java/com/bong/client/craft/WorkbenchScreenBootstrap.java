package com.bong.client.craft;

import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import net.minecraft.client.MinecraftClient;

/**
 * 制作台权威入口：携带工位身份打开统一制作窗口。
 *
 * <p>监听 {@code workbench_open} payload 并打开 Inspect 工作台。
 * 不需要 keybinding（制作台通过右键方块触发）。</p>
 */
public final class WorkbenchScreenBootstrap {
    private WorkbenchScreenBootstrap() {}

    /**
     * 返回处理 {@code workbench_open} payload 的 handler，
     * 由 {@link com.bong.client.network.ServerDataRouter} 注册。
     */
    public static ServerDataHandler handler() {
        return WorkbenchScreenBootstrap::handleOpen;
    }

    private static ServerDataDispatch handleOpen(ServerDataEnvelope envelope) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.player == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring workbench_open: no player");
        }
        CraftContext context;
        try {
            var payload = envelope.payload();
            int entityId = payload.get("entity_id").getAsBigDecimal().intValueExact();
            var position = payload.getAsJsonArray("position");
            if (entityId < 0 || position.size() != 3) throw new IllegalArgumentException("invalid workbench target");
            context = new CraftContext(new CraftContext.Workbench(entityId,
                position.get(0).getAsBigDecimal().intValueExact(),
                position.get(1).getAsBigDecimal().intValueExact(),
                position.get(2).getAsBigDecimal().intValueExact()));
        } catch (RuntimeException failure) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring invalid workbench target");
        }
        var connection = client.getNetworkHandler();
        var world = client.world;
        client.execute(() -> {
            if (connection == client.getNetworkHandler() && world == client.world) {
                CraftScreenBootstrap.open(client, context);
            }
        });
        return ServerDataDispatch.handled(envelope.type(),
            "Opened craft window for workbench");
    }
}
