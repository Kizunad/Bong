package com.bong.client.agentui;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;

/**
 * plan-agent-ui-data-v1 P1 — 天道 UI 客户端 tick 接线。
 *
 * <p>在 {@link ClientTickEvents#END_CLIENT_TICK} 每游戏 tick 驱动 {@link AgentUiScreen#tickLocalTimeout}，
 * 实现 timeout_ticks + 20t 宽限后本地自动收起面板。
 *
 * <p>由 {@link com.bong.client.BongClient#onInitializeClient()} 调用一次。
 */
public final class AgentUiBootstrap {

    private AgentUiBootstrap() {}

    /**
     * 注册客户端 tick 钩子，驱动 {@link AgentUiStore} 中活跃 screen 的本地超时计数。
     *
     * <p>每 tick：若 {@link AgentUiStore#getActive()} 非 null，
     * 用当前世界时间（{@code world.getTime()}）调用 {@code tickLocalTimeout}。
     * world 不可用时跳过（tick 但不推进）。
     */
    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(AgentUiBootstrap::onEndClientTick);
    }

    private static void onEndClientTick(MinecraftClient client) {
        AgentUiScreen active = AgentUiStore.getActive();
        if (active == null) {
            return;
        }
        if (client.world == null) {
            return;
        }
        long currentTick = client.world.getTime();
        active.tickLocalTimeout(currentTick);
    }
}
