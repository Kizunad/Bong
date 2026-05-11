package com.bong.client.spirittreasure;

import java.util.List;

public record SpiritTreasureState(
    String templateId,
    String displayName,
    long instanceId,
    boolean equipped,
    boolean passiveActive,
    double affinity,
    boolean sleeping,
    String sourceSect,
    String iconTexture,
    List<SpiritTreasurePassive> passiveEffects
) {
    public SpiritTreasureState {
        templateId = sanitize(templateId);
        displayName = sanitize(displayName);
        sourceSect = sourceSect == null ? "" : sourceSect.trim();
        iconTexture = sanitize(iconTexture);
        affinity = Math.max(0.0, Math.min(1.0, affinity));
        passiveEffects = List.copyOf(passiveEffects == null ? List.of() : passiveEffects);
    }

    public String sourceLine() {
        return sourceSect.isBlank() ? "来源未明" : "出自" + sourceSect;
    }

    private static String sanitize(String value) {
        return value == null ? "" : value.trim();
    }
}
