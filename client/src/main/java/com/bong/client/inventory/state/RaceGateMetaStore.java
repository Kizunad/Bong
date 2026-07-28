package com.bong.client.inventory.state;

import com.bong.client.inventory.model.RaceGate;

import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * plan-race-system-v1 P3c — 种族门元数据表（{@code race_gate_meta} payload）的 client 缓存。
 *
 * <p>两张表（仿 {@link BodyPlanLayoutStore} 的 volatile + listener 惯例）：
 * <ul>
 *   <li>{@code itemWearerRace}：item template_id → {@link RaceGate}（**装备门**，判当前形态身份）</li>
 *   <li>{@code techniqueRequiredRace}：technique skill_id → {@link RaceGate}（**功法门**，判本体身份）</li>
 * </ul>
 *
 * <p>表里**只有非 any 条目**（server 只下发非 Any——any 是默认）。因此
 * {@link #gateForItem}/{@link #gateForTechnique} 查不到（返回 {@code null}）即 = any = 恒放行。
 *
 * <p>判定逻辑（含 fail-closed）在 {@link RaceGateEval}，本类只做纯缓存 + 变更通知。
 * 内容静态（与身份无关），server join 首帧一次性下发；client 身份变化（易形）时
 * **不需重发**——用 {@link PlayerRaceIdentityStore} 最新身份对同一张表重判即可，
 * 故本 store 变更后同样通知监听者触发 UI 重刷。
 */
public final class RaceGateMetaStore {
    private static volatile Map<String, RaceGate> itemWearerRace = Map.of();
    private static volatile Map<String, RaceGate> techniqueRequiredRace = Map.of();
    /** 是否已收到过一份 {@code race_gate_meta} payload（区别于"收到但两表皆空"）。 */
    private static volatile boolean received;
    private static final List<Runnable> listeners = new CopyOnWriteArrayList<>();

    private RaceGateMetaStore() {}

    /** 整表替换（收到新 {@code race_gate_meta} payload）。any 条目理应不出现，出现也无害（isAny 恒放行）。 */
    public static void replace(Map<String, RaceGate> items, Map<String, RaceGate> techniques) {
        itemWearerRace = items == null ? Map.of() : Map.copyOf(items);
        techniqueRequiredRace = techniques == null ? Map.of() : Map.copyOf(techniques);
        received = true;
        notifyListeners();
    }

    /** 该 item 的装备门；{@code null} = 表内无条目 = any = 恒放行。 */
    public static RaceGate gateForItem(String templateId) {
        return templateId == null ? null : itemWearerRace.get(templateId);
    }

    /** 该 technique 的功法门；{@code null} = 表内无条目 = any = 恒放行。 */
    public static RaceGate gateForTechnique(String skillId) {
        return skillId == null ? null : techniqueRequiredRace.get(skillId);
    }

    /** 是否已收到过一份 payload（诊断 / 测试用；判定不依赖它，靠 gateFor* 的 null 语义）。 */
    public static boolean hasReceived() {
        return received;
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
     * 断线时清空本会话 gate 元数据并恢复“尚未收到 payload”标记，保留 UI listener wiring。
     */
    public static void clearOnDisconnect() {
        itemWearerRace = Map.of();
        techniqueRequiredRace = Map.of();
        received = false;
        notifyListeners();
    }

    public static void resetForTests() {
        itemWearerRace = Map.of();
        techniqueRequiredRace = Map.of();
        received = false;
        listeners.clear();
    }
}
