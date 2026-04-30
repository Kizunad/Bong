package com.bong.client.visual.realm_vision;

public final class ClientRenderDistanceAdvisor {
    public static final int WARN_BELOW_CHUNKS = 12;
    public static final int RECOMMENDED_CHUNKS = 16;

    private ClientRenderDistanceAdvisor() {
    }

    public static boolean shouldWarn(int clientViewDistanceChunks) {
        return clientViewDistanceChunks > 0 && clientViewDistanceChunks < WARN_BELOW_CHUNKS;
    }

    public static String warningText() {
        return "修仙世界推荐 render distance >= " + RECOMMENDED_CHUNKS + " chunks，否则视野体验受限";
    }
}
