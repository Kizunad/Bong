package com.bong.client.combat;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * plan-weapon-v1 §9：客户端武器装备状态集中存储。
 *
 * <p>按 slot(main_hand / off_hand / two_hand)索引当前装备的 {@link EquippedWeapon}。
 * {@code WeaponEquippedHandler} 收到 {@code weapon_equipped} payload 时写入;
 * {@code WeaponHotbarHudPlanner} 每帧读取画武器槽。
 *
 * <p>线程安全:{@link ConcurrentHashMap},handler 在 client tick 线程写入,
 * HudRenderCallback 在主渲染线程读取。
 */
public final class WeaponEquippedStore {
    private static final Map<String, EquippedWeapon> snapshots = new ConcurrentHashMap<>();

    private WeaponEquippedStore() {
    }

    public static EquippedWeapon get(String slot) {
        return snapshots.get(slot);
    }

    /** weapon == null 表示该 slot 被清空(卸下 / broken)。 */
    public static void putOrClear(String slot, EquippedWeapon weapon) {
        if (weapon == null) {
            snapshots.remove(slot);
        } else {
            snapshots.put(slot, weapon);
        }
    }

    public static void resetForTests() {
        snapshots.clear();
    }
}
