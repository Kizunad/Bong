package com.bong.client.inventory.state;

import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;

import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * plan-race-system-v1 P2b — 以 {@code body_plan_id} 为键的多快照缓存（仿
 * {@link MeridianStateStore} 的 volatile + 订阅模式，但一个实体本体只有一个"当前"
 * plan，其余 plan 的 layout 只是缓存以便实体换 race / 玩家再次登录不同种族角色时
 * 不必等重新下发）。
 *
 * <p>两个来源互相独立到达，天然存在首帧竞态：
 * <ul>
 *   <li>{@code body_plan_layout} payload → {@link #putLayout(BodyPlanLayout)}（按 id 建缓存）</li>
 *   <li>{@code cultivation_detail.body_plan_id} → {@link #setCurrentPlanId(String)}（更新"当前"指针）</li>
 * </ul>
 * 谁先到都不crash：{@link #current()} 在缓存缺失时返回 {@code null}，消费方（
 * {@code MiniBodyHudPlanner} / {@code BodyInspectComponent} / {@code WoundLayerBinding}）
 * 负责对 {@code null} 做仅视觉 fallback（不做 gate 判定，见 plan §P2 / §P3 分工）。
 */
public final class BodyPlanLayoutStore {
    private static final Map<String, BodyPlanLayout> cache = new ConcurrentHashMap<>();
    private static volatile String currentPlanId;
    private static final List<Consumer<BodyPlanLayout>> listeners = new CopyOnWriteArrayList<>();

    private BodyPlanLayoutStore() {}

    /** 缓存一份 layout（按其自带的 {@code bodyPlanId} 为键）；若恰好是当前 plan，通知监听者。 */
    public static void putLayout(BodyPlanLayout layout) {
        if (layout == null || layout.bodyPlanId() == null || layout.bodyPlanId().isEmpty()) {
            return;
        }
        cache.put(layout.bodyPlanId(), layout);
        if (layout.bodyPlanId().equals(currentPlanId)) {
            notifyListeners();
        }
    }

    /** 更新"当前"plan 指针（来自 {@code cultivation_detail.body_plan_id}）；变化时通知监听者。 */
    public static void setCurrentPlanId(String planId) {
        if (Objects.equals(planId, currentPlanId)) {
            return;
        }
        currentPlanId = planId;
        notifyListeners();
    }

    public static String currentPlanId() {
        return currentPlanId;
    }

    /** 当前 plan 对应的 layout；未知 plan id 或尚未收到该 layout 时返回 {@code null}。 */
    public static BodyPlanLayout current() {
        String id = currentPlanId;
        return id == null ? null : cache.get(id);
    }

    /** 按 plan id 直接查缓存（不受"当前"指针影响）。 */
    public static BodyPlanLayout byId(String planId) {
        return planId == null ? null : cache.get(planId);
    }

    public static void addListener(Consumer<BodyPlanLayout> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<BodyPlanLayout> listener) {
        listeners.remove(listener);
    }

    private static void notifyListeners() {
        BodyPlanLayout layout = current();
        for (Consumer<BodyPlanLayout> listener : listeners) {
            listener.accept(layout);
        }
    }

    public static void resetForTests() {
        cache.clear();
        currentPlanId = null;
        listeners.clear();
    }
}
