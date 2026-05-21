package com.bong.client.craft;

import com.bong.client.BongClient;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import net.minecraft.client.MinecraftClient;

/**
 * plan-workbench-recipes-v1 P3.1 -- WorkbenchScreen 启动器。
 *
 * <p>监听 {@code workbench_open} payload 并打开 {@link WorkbenchScreen}。
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
        client.execute(() -> {
            if (!(client.currentScreen instanceof WorkbenchScreen)) {
                client.setScreen(new WorkbenchScreen());
            }
        });
        BongClient.LOGGER.info("Opened WorkbenchScreen from workbench_open payload");
        return ServerDataDispatch.handled(envelope.type(),
            "Opened WorkbenchScreen");
    }
}
