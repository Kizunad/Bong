package com.bong.client.skill;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * plan-skill-v1 §8 客户端 skill 全局快照。网络 handler 调用 {@link #replace} /
 * {@link #updateEntry}；UI 组件订阅变更。
 *
 * <p>botany/lingtian 的采药单项视图也从本 store 的 {@link SkillId#HERBALISM} 条目派生，
 * 不再维护独立 skill 状态。
 */
public final class SkillSetStore {
    private static volatile SkillSetSnapshot snapshot = SkillSetSnapshot.empty();
    private static final List<Consumer<SkillSetSnapshot>> listeners = new CopyOnWriteArrayList<>();

    private SkillSetStore() {}

    public static SkillSetSnapshot snapshot() {
        return snapshot;
    }

    public static void replace(SkillSetSnapshot next) {
        snapshot = next == null ? SkillSetSnapshot.empty() : next;
        for (Consumer<SkillSetSnapshot> l : listeners) l.accept(snapshot);
    }

    /**
     * 单 skill 条目更新的语义糖 —— 网络 handler 主要走这条路径。
     */
    public static void updateEntry(SkillId id, SkillSetSnapshot.Entry entry) {
        if (id == null || entry == null) return;
        replace(snapshot.withSkill(id, entry));
    }

    public static void addListener(Consumer<SkillSetSnapshot> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<SkillSetSnapshot> listener) {
        listeners.remove(listener);
    }

    public static void clearOnDisconnect() {
        snapshot = SkillSetSnapshot.empty();
        for (Consumer<SkillSetSnapshot> l : listeners) l.accept(snapshot);
    }

    public static void resetForTests() {
        snapshot = SkillSetSnapshot.empty();
        listeners.clear();
    }
}
