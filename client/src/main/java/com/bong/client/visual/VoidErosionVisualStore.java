package com.bong.client.visual;

import java.util.concurrent.atomic.AtomicReference;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 — 客户端虚蚀视觉状态存储。
 *
 * 由 {@link VoidErosionVisualHandler} 写入，由 HUD overlay 渲染器读取。
 * 跨帧共享，使用 AtomicReference 保证可见性（Minecraft 主线程写+读均可安全访问）。
 */
public final class VoidErosionVisualStore {

    /** 当前虚蚀状态快照（null = 未收到任何 payload）。 */
    private static final AtomicReference<State> CURRENT = new AtomicReference<>(null);

    private VoidErosionVisualStore() {}

    /**
     * 替换当前虚蚀视觉状态（由 VoidErosionVisualHandler 在 Minecraft 主线程调用）。
     *
     * @param entityId          实体 UUID / wire id
     * @param stage             虚蚀阶段（0-4）
     * @param cumulativeErosion 累计虚蚀值
     * @param ambientActive     常驻涡流是否激活
     * @param modelAlpha        模型透明度（0.0~1.0，阶段 4 = 0.4）
     * @param soundDistortion   声音扭曲 overlay 是否激活（阶段 3+）
     */
    public static void replace(
            String entityId,
            int stage,
            double cumulativeErosion,
            boolean ambientActive,
            float modelAlpha,
            boolean soundDistortion
    ) {
        CURRENT.set(new State(entityId, stage, cumulativeErosion, ambientActive, modelAlpha, soundDistortion));
    }

    /** 返回当前快照，或 null（如果尚未收到 server payload）。 */
    public static State snapshot() {
        return CURRENT.get();
    }

    /** 重置（断线时清理）。 */
    public static void reset() {
        CURRENT.set(null);
    }

    /**
     * 虚蚀视觉状态快照（不可变记录）。
     */
    public record State(
            String entityId,
            int stage,
            double cumulativeErosion,
            boolean ambientActive,
            /** 玩家模型透明度，1.0 = 完全不透明，0.4 = 阶段 4 最低值。 */
            float modelAlpha,
            /** 声音扭曲 HUD overlay 是否激活（阶段 3+）。 */
            boolean soundDistortionActive
    ) {}
}
