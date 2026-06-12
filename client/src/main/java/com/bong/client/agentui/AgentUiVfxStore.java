package com.bong.client.agentui;

import org.jetbrains.annotations.Nullable;

/**
 * plan-agent-ui-data-v1 P3 — 天道动态面板 VFX 状态单例存储。
 *
 * <p>由 {@link AgentUiScreen#init()} 在面板打开时写入；
 * 由 {@link AgentUiVfxPlanner} 每帧读取生成 HudRenderCommand；
 * 面板关闭时清除（{@link #clear()}）。
 */
public final class AgentUiVfxStore {
    private AgentUiVfxStore() {}

    @Nullable
    private static volatile AgentUiVfxState current = null;

    /** 设置当前 VFX 状态（面板打开时调用）。 */
    public static void setActive(AgentUiVfxState state) {
        current = state;
    }

    /** 获取当前 VFX 状态（null = 无活跃面板 VFX）。 */
    @Nullable
    public static AgentUiVfxState getActive() {
        return current;
    }

    /** 清除 VFX 状态（面板关闭时调用）。 */
    public static void clear() {
        current = null;
    }
}
