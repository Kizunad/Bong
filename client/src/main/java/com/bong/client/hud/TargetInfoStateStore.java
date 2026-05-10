package com.bong.client.hud;

import net.minecraft.entity.Entity;

public final class TargetInfoStateStore {
    private static volatile TargetInfoState snapshot = TargetInfoState.empty();

    private TargetInfoStateStore() {
    }

    public static TargetInfoState snapshot() {
        return snapshot;
    }

    public static void observeEntity(Entity entity, long observedAtMillis) {
        TargetInfoState next = TargetInfoState.fromEntity(entity, observedAtMillis);
        if (!next.isEmpty()) {
            snapshot = next;
        }
    }

    public static void replaceForTests(TargetInfoState next) {
        snapshot = next == null ? TargetInfoState.empty() : next;
    }

    public static void resetForTests() {
        snapshot = TargetInfoState.empty();
    }
}
