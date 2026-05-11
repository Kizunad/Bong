package com.bong.client.spirittreasure;

public record SpiritTreasureDialogue(
    String requestId,
    String characterId,
    String treasureId,
    String displayName,
    String text,
    String tone,
    double affinityDelta,
    String zone,
    long receivedAtMs
) {
    public SpiritTreasureDialogue {
        requestId = sanitize(requestId);
        characterId = sanitize(characterId);
        treasureId = sanitize(treasureId);
        displayName = sanitize(displayName);
        text = sanitize(text);
        tone = sanitize(tone);
        zone = sanitize(zone);
        affinityDelta = Math.max(-0.1, Math.min(0.1, affinityDelta));
    }

    private static String sanitize(String value) {
        return value == null ? "" : value.trim();
    }
}
