package com.bong.client.visual.realm_vision;

public record RealmVisionState(
    RealmVisionCommand current,
    RealmVisionCommand previous,
    int transitionTicks,
    int elapsedTicks,
    long startedAtTick,
    int serverViewDistanceChunks
) {
    public static RealmVisionState empty() {
        return new RealmVisionState(null, null, 0, 0, 0L, 0);
    }

    public boolean isEmpty() {
        return current == null;
    }
}
