package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

public final class NpcBubbleHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "npc_bubble";
    public static final int VERSION = 1;

    private NpcBubbleHandler() {
    }

    public static boolean handle(String jsonPayload, int payloadSizeBytes, long nowMillis) {
        if (jsonPayload == null || payloadSizeBytes < 0 || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return false;
        }
        JsonObject root;
        try {
            root = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (RuntimeException exception) {
            return false;
        }
        if (intField(root, "v", -1) != VERSION || !"npc_bubble".equals(stringField(root, "type", ""))) {
            return false;
        }
        int entityId = intField(root, "entity_id", Integer.MIN_VALUE);
        if (entityId < 0) {
            return false;
        }
        String text = stringField(root, "text", "");
        if (text.isBlank()) {
            return false;
        }
        int durationTicks = intField(root, "duration_ticks", NpcDialogueBubbleRenderer.durationTicksForText(text));
        String bubbleType = stringField(root, "bubble_type", "greeting");
        NpcMetadata metadata = NpcMetadataStore.get(entityId);
        NpcDialogueBubbleRenderer.show(new NpcDialogueBubbleRenderer.Bubble(
            entityId,
            text,
            bubbleType,
            metadata == null ? "unknown" : metadata.archetype(),
            Math.max(60, Math.min(120, durationTicks)) * 50L,
            nowMillis
        ));
        NpcInteractionLogStore.record(new NpcInteractionLogEntry(
            entityId,
            metadata == null ? "未知 NPC" : metadata.displayName(),
            metadata == null ? "unknown" : metadata.archetype(),
            bubbleType,
            nowMillis
        ));
        return true;
    }

    private static int intField(JsonObject root, String fieldName, int fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            return root.get(fieldName).getAsInt();
        } catch (RuntimeException exception) {
            return fallback;
        }
    }

    private static String stringField(JsonObject root, String fieldName, String fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            String value = root.get(fieldName).getAsString();
            return value == null || value.isBlank() ? fallback : value.trim();
        } catch (RuntimeException exception) {
            return fallback;
        }
    }
}
