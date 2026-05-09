package com.bong.client.hud;

public record AnqiHudState(
    float aimProgress,
    float chargeProgress,
    int echoCount,
    String abrasionContainer,
    float abrasionQiPayload,
    long expiresAtMillis
) {
    public static AnqiHudState empty() {
        return new AnqiHudState(0f, 0f, 0, "", 0f, 0L);
    }

    public boolean active(long nowMillis) {
        return expiresAtMillis > nowMillis
            && (aimProgress > 0f || chargeProgress > 0f || echoCount > 0 || hasAbrasionContainer());
    }

    public boolean hasAbrasionContainer() {
        return abrasionContainer != null && !abrasionContainer.isBlank();
    }

    public static AnqiHudState aim(float progress, long nowMillis, long durationMillis) {
        return new AnqiHudState(clamp01(progress), 0f, 0, "", 0f, nowMillis + Math.max(1L, durationMillis));
    }

    public static AnqiHudState charge(float progress, long nowMillis, long durationMillis) {
        return new AnqiHudState(0f, clamp01(progress), 0, "", 0f, nowMillis + Math.max(1L, durationMillis));
    }

    public static AnqiHudState echo(int count, long nowMillis, long durationMillis) {
        return new AnqiHudState(0f, 0f, Math.max(0, count), "", 0f, nowMillis + Math.max(1L, durationMillis));
    }

    public static AnqiHudState abrasion(String container, float qiPayload, long nowMillis, long durationMillis) {
        return new AnqiHudState(0f, 0f, 0, container == null ? "" : container, Math.max(0f, qiPayload), nowMillis + Math.max(1L, durationMillis));
    }

    static float clamp01(float value) {
        if (!Float.isFinite(value)) return 0f;
        return Math.max(0f, Math.min(1f, value));
    }
}
