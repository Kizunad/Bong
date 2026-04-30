package com.bong.client.visual.realm_vision;

public final class ClientRenderDistanceAdvisorState {
    private static volatile boolean warned;

    private ClientRenderDistanceAdvisorState() {
    }

    public static boolean markWarnedIfNeeded(int clientViewDistanceChunks) {
        if (warned || !ClientRenderDistanceAdvisor.shouldWarn(clientViewDistanceChunks)) {
            return false;
        }
        warned = true;
        return true;
    }

    public static void resetForTests() {
        warned = false;
    }
}
