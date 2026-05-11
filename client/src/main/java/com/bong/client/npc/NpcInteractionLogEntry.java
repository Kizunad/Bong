package com.bong.client.npc;

public record NpcInteractionLogEntry(
    int entityId,
    String displayName,
    String archetype,
    String interactionType,
    long observedAtMillis
) {
    public NpcInteractionLogEntry {
        displayName = clean(displayName, "未知 NPC");
        archetype = clean(archetype, "unknown");
        interactionType = clean(interactionType, "interaction");
        observedAtMillis = Math.max(0L, observedAtMillis);
    }

    private static String clean(String value, String fallback) {
        if (value == null || value.isBlank()) {
            return fallback;
        }
        return value.trim();
    }
}
