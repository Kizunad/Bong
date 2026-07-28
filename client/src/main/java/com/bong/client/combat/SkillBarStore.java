package com.bong.client.combat;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/** Client-side mirror for the 1-9 combat skill bar. */
public final class SkillBarStore {
    public static final int NO_SELECTED_SLOT = -1;
    private static volatile SkillBarConfig snapshot = SkillBarConfig.empty();
    private static volatile int selectedSlot = NO_SELECTED_SLOT;
    private static final List<Consumer<SkillBarConfig>> listeners = new CopyOnWriteArrayList<>();

    private SkillBarStore() {
    }

    public static SkillBarConfig snapshot() {
        return snapshot;
    }

    public static void replace(SkillBarConfig next) {
        snapshot = next == null ? SkillBarConfig.empty() : next;
        clearInvalidSelectedSlot();
        notifyListeners();
    }

    public static void updateSlot(int index, SkillBarEntry entry) {
        snapshot = snapshot.withSlot(index, entry);
        clearInvalidSelectedSlot();
        notifyListeners();
    }

    public static void addListener(Consumer<SkillBarConfig> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<SkillBarConfig> listener) {
        listeners.remove(listener);
    }

    public static int findSkill(String skillId) {
        return snapshot.findSkill(skillId);
    }

    public static int selectedSlot() {
        return selectedSlot;
    }

    public static void setSelectedSlot(int slot) {
        selectedSlot = slot >= 0 && slot < SkillBarConfig.SLOT_COUNT ? slot : NO_SELECTED_SLOT;
    }

    public static void clearSelectedSlot() {
        selectedSlot = NO_SELECTED_SLOT;
    }

    /** Clears bindings and selection while retaining long-lived listeners. */
    public static void clearOnDisconnect() {
        replace(SkillBarConfig.empty());
    }

    public static void resetForTests() {
        snapshot = SkillBarConfig.empty();
        selectedSlot = NO_SELECTED_SLOT;
        listeners.clear();
    }

    private static void clearInvalidSelectedSlot() {
        SkillBarEntry selectedEntry = snapshot.slot(selectedSlot);
        if (selectedEntry == null || selectedEntry.kind() != SkillBarEntry.Kind.ITEM) {
            selectedSlot = NO_SELECTED_SLOT;
        }
    }

    private static void notifyListeners() {
        for (Consumer<SkillBarConfig> listener : listeners) {
            listener.accept(snapshot);
        }
    }
}
