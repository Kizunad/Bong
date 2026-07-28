package com.bong.client.inventory.state;

import com.bong.client.inventory.model.MorphEntry;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * plan-race-system-v1 PR-5b — {@code morph_state} payload 的 client 缓存
 * （仿 {@link RaceGateMetaStore} 的 volatile + listener 惯例，但 key 是
 * **per-entity**（{@code entity_id}），语义对齐 daozhan/spider 伪装表的
 * per-entity 缓存，而非 {@link PlayerRaceIdentityStore} 那种 per-self 单值）。
 *
 * <p>两种 {@code mode} 的应用语义：
 * <ul>
 *   <li>{@code full}（join 首帧 / 周期性重发）：{@link #applyFull} 整表替换——
 *       表内只包含当前处于易形态的实体，{@code clear+putAll} 语义。</li>
 *   <li>{@code delta}（易形瞬间 / 解除瞬间半径广播）：{@link #applyDelta} 逐条应用——
 *       {@code active=true} 则 put，{@code active=false} 则 remove。</li>
 * </ul>
 *
 * <p>查表 miss（{@link #morphOf} 返回空）= 该实体未易形 = 渲染走原版模型，
 * 与 {@code RaceGateMetaStore} 的"查不到=any"同一惯例（缺省语义安全）。
 */
public final class MorphStateStore {
    private static volatile Map<Integer, MorphEntry> morphed = Map.of();
    private static final List<Runnable> listeners = new CopyOnWriteArrayList<>();

    private MorphStateStore() {}

    /** {@code mode="full"}：整表替换。{@code null} 视为空表。 */
    public static void applyFull(Map<Integer, MorphEntry> active) {
        morphed = active == null ? Map.of() : Map.copyOf(active);
        notifyListeners();
    }

    /**
     * {@code mode="delta"}：单条应用。{@code entry == null} 表示
     * {@code active=false}（解除易形），从表中移除该 {@code entityId}；
     * 非空表示插入/更新。
     */
    public static void applyDelta(int entityId, MorphEntry entry) {
        Map<Integer, MorphEntry> copy = new HashMap<>(morphed);
        if (entry == null) {
            copy.remove(entityId);
        } else {
            copy.put(entityId, entry);
        }
        morphed = Map.copyOf(copy);
        notifyListeners();
    }

    /** 该实体当前易形形态；空 = 未易形（原版模型）。 */
    public static Optional<MorphEntry> morphOf(int entityId) {
        return Optional.ofNullable(morphed.get(entityId));
    }

    public static void addListener(Runnable listener) {
        listeners.add(listener);
    }

    public static void removeListener(Runnable listener) {
        listeners.remove(listener);
    }

    private static void notifyListeners() {
        for (Runnable listener : listeners) {
            listener.run();
        }
    }

    /**
     * 断线时清空本会话易形表，保留长期渲染 listener wiring。
     */
    public static void clearOnDisconnect() {
        morphed = Map.of();
        notifyListeners();
    }

    public static void resetForTests() {
        morphed = Map.of();
        listeners.clear();
    }
}
