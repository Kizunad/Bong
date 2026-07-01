package com.bong.client.coffin;

import net.minecraft.util.math.BlockPos;

import java.util.Optional;

/**
 * F9 跨层修复 — 出生引导棺权威坐标的单值缓存。
 *
 * <p>Server 在玩家 join 时广播 {@code tutorial_coffin_pos}
 * （{@link com.bong.client.network.TutorialCoffinPosHandler}），本 store 缓存最近一次
 * 收到的精确坐标，供
 * {@link com.bong.client.mixin.MixinClientPlayerInteractionManagerAlchemy} 判定右键的
 * 方块是否为出生引导棺——取代此前硬编码的 {@code |x|<=8, y∈[60,90], |z|<=8} 判定盒
 * （spawn 迁移后就会失配，真棺打不开）。
 *
 * <p>仿 {@code InventoryStateStore} 单值 volatile store 模式。</p>
 */
public final class TutorialCoffinPosStore {
    private static volatile BlockPos pos;

    private TutorialCoffinPosStore() {}

    /** 收到 {@code tutorial_coffin_pos} payload 时调用。 */
    public static void set(BlockPos next) {
        pos = next;
    }

    /** 当前缓存的权威坐标；join 后尚未收到广播、或断线清理后为空。 */
    public static Optional<BlockPos> snapshot() {
        return Optional.ofNullable(pos);
    }

    /** 断线时调用，防止旧 server 的坐标跨 session 续命（不同 server 的 spawn 棺位置不同）。 */
    public static void clearOnDisconnect() {
        pos = null;
    }

    public static void resetForTests() {
        pos = null;
    }
}
