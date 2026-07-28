package com.bong.client.visual;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 — 客户端虚蚀视觉状态存储。
 *
 * 由 {@link VoidErosionVisualHandler} 写入，由 HUD overlay 渲染器读取。
 * 存储为 per-entity Map，键为 server wire entityId（"offline:{username}"/"char:{bits}"），
 * 每个玩家实体独立管理自己的虚蚀状态，避免多人游戏中 alpha 串扰。
 *
 * 跨帧共享，ConcurrentHashMap 保证多线程写入/读取安全（Minecraft 渲染线程读取，
 * 主线程写入）。
 */
public final class VoidErosionVisualStore {

    /** per-entity 虚蚀状态（键 = server wire entityId）。 */
    private static final ConcurrentHashMap<String, State> STATES = new ConcurrentHashMap<>();

    private VoidErosionVisualStore() {}

    /**
     * 替换指定实体的虚蚀视觉状态（由 VoidErosionVisualHandler 在 Minecraft 主线程调用）。
     *
     * @param entityId           实体 UUID / wire id（作为 map key）
     * @param stage              虚蚀阶段（0-4）
     * @param cumulativeErosion  累计虚蚀值
     * @param ambientActive      常驻涡流是否激活
     * @param modelAlpha         模型透明度（0.0~1.0，阶段 4 = 0.4）
     * @param voidDistortion     虚蚀扭曲视觉 overlay 是否激活（阶段 3+，无音频逻辑）
     */
    public static void replace(
            String entityId,
            int stage,
            double cumulativeErosion,
            boolean ambientActive,
            float modelAlpha,
            boolean voidDistortion
    ) {
        STATES.put(entityId, new State(entityId, stage, cumulativeErosion, ambientActive, modelAlpha, voidDistortion));
    }

    /**
     * 返回指定实体的当前快照，或 null（如果尚未收到该实体的 server payload）。
     *
     * @param entityId server wire entityId（"offline:{username}"/"char:{bits}"）
     */
    public static State snapshotForEntity(String entityId) {
        return STATES.get(entityId);
    }

    /**
     * 返回当前所有实体状态的只读视图（用于 HUD、调试等需要遍历所有玩家的场景）。
     */
    public static Map<String, State> allSnapshots() {
        return java.util.Collections.unmodifiableMap(STATES);
    }

    /**
     * 返回 map 中第一个（任意）快照，或 null。
     *
     * <p>仅保留用于向后兼容——本地单人场景只有一个玩家时等价于旧的单槽 snapshot()。
     * 多人场景请改用 {@link #snapshotForEntity(String)} 或 {@link #allSnapshots()}。
     *
     * @deprecated 多人场景请用 {@link #snapshotForEntity(String)}；
     *             单人场景此方法可用，但不保证稳定性。
     */
    @Deprecated
    public static State snapshot() {
        if (STATES.isEmpty()) return null;
        return STATES.values().iterator().next();
    }

    /**
     * 清除指定实体的状态（断线时调用）。
     *
     * @param entityId server wire entityId
     */
    public static void remove(String entityId) {
        STATES.remove(entityId);
    }

    /** 重置所有实体状态（断线/重连时清理）。 */
    public static void reset() {
        STATES.clear();
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
            /**
             * 虚蚀扭曲视觉 HUD overlay 是否激活（阶段 3+）。
             * 仅控制客户端视觉 vignette；音效经独立 bong:audio/play channel 触发，
             * 不在本字段范围内（fix5：原 soundDistortionActive，去掉 sound 字样防名实不符）。
             */
            boolean voidDistortionActive
    ) {}
}
