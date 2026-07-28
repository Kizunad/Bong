package com.bong.client.combat;

import java.util.concurrent.atomic.AtomicReference;

/**
 * plan-shield-block-v1 §P3：客户端盾牌装备状态集中存储（独立于 WeaponEquippedStore）。
 *
 * <p>盾牌语义上不是武器，不污染 {@link WeaponEquippedStore}。只记录 off_hand 盾槽状态：
 * <ul>
 *   <li>装备盾牌：{@link #equip(EquippedShield)} 写入快照</li>
 *   <li>卸下 / 破盾：{@link #clear()} 清空快照</li>
 *   <li>HUD 每帧读取：{@link #snapshot()} 取当前快照（可为 null）</li>
 * </ul>
 *
 * <p>线程安全：{@link AtomicReference}，handler 在 client tick 线程写，HUD 在渲染线程读。
 */
public final class EquippedShieldStore {
    private static final AtomicReference<EquippedShield> current = new AtomicReference<>(null);

    private EquippedShieldStore() {
    }

    /** 装备盾牌，写入快照。{@code shield} 为 null 等价于 {@link #clear()}。 */
    public static void equip(EquippedShield shield) {
        current.set(shield);
    }

    /**
     * 卸下或破盾后清空快照。
     * 若 {@code instanceId} 与当前快照不匹配（已被新盾牌覆盖），则跳过清空，避免竞态覆写。
     *
     * @param instanceId 期望清除的盾牌 instance_id；传 -1 则无条件清空（卸下场景）
     */
    public static void clearIfInstance(long instanceId) {
        if (instanceId < 0) {
            current.set(null);
            return;
        }
        current.compareAndSet(
            /* 仅当 current 等于持有该 instanceId 的快照时清空 */
            currentIfMatches(instanceId),
            null
        );
    }

    /** 无条件清空（卸下 / 断开连接）。 */
    public static void clear() {
        current.set(null);
    }

    /** 返回当前快照；未装备时为 null。 */
    public static EquippedShield snapshot() {
        return current.get();
    }

    /** 仅测试用。生产代码勿调用。 */
    public static void resetForTests() {
        current.set(null);
    }

    // ---- 私有辅助 -------------------------------------------------------

    private static EquippedShield currentIfMatches(long instanceId) {
        EquippedShield s = current.get();
        return (s != null && s.instanceId() == instanceId) ? s : null;
    }
}
