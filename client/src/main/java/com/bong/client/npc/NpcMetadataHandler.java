package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public final class NpcMetadataHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "npc_metadata";
    public static final int VERSION = 1;

    private NpcMetadataHandler() {
    }

    public static boolean handle(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null || payloadSizeBytes < 0 || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return false;
        }
        JsonObject root;
        try {
            root = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (RuntimeException exception) {
            return false;
        }
        if (intField(root, "v", -1) != VERSION) {
            return false;
        }
        if (!"npc_metadata".equals(stringField(root, "type", ""))) {
            return false;
        }
        int entityId = intField(root, "entity_id", Integer.MIN_VALUE);
        if (entityId == Integer.MIN_VALUE || entityId < 0) {
            return false;
        }

        NpcMetadataStore.upsert(new NpcMetadata(
            entityId,
            stringField(root, "archetype", "unknown"),
            stringField(root, "realm", "未知"),
            nullableString(root, "faction_name"),
            nullableString(root, "faction_rank"),
            intField(root, "reputation_to_player", 0),
            stringField(root, "display_name", ""),
            stringField(root, "age_band", "正值壮年"),
            stringField(root, "greeting_text", "对方沉默地看着你。"),
            nullableString(root, "qi_hint"),
            doubleField(root, "hp_ratio", 1.0),
            doubleField(root, "qi_ratio", 0.0),
            parseEquipment(root),
            parseTechniques(root),
            parseTradeOffers(root)
        ));
        return true;
    }

    // ── equipment parsing ────────────────────────────────────────────────────

    static Map<String, NpcEquipSlotData> parseEquipment(JsonObject root) {
        if (!root.has("equipment") || root.get("equipment").isJsonNull()) {
            return Map.of();
        }
        JsonElement element = root.get("equipment");
        if (!element.isJsonObject()) {
            return Map.of();
        }
        JsonObject equipObj = element.getAsJsonObject();
        Map<String, NpcEquipSlotData> result = new HashMap<>();
        for (String slotName : equipObj.keySet()) {
            JsonElement slotElem = equipObj.get(slotName);
            if (slotElem == null || slotElem.isJsonNull() || !slotElem.isJsonObject()) {
                continue;
            }
            JsonObject slot = slotElem.getAsJsonObject();
            result.put(slotName, new NpcEquipSlotData(
                stringField(slot, "template_id", ""),
                stringField(slot, "display_name", ""),
                intField(slot, "quality_tier", 0),
                doubleField(slot, "durability_ratio", 1.0)
            ));
        }
        return result.isEmpty() ? Map.of() : Map.copyOf(result);
    }

    // ── techniques parsing ───────────────────────────────────────────────────

    static List<NpcTechniqueData> parseTechniques(JsonObject root) {
        if (!root.has("techniques") || root.get("techniques").isJsonNull()) {
            return List.of();
        }
        JsonElement element = root.get("techniques");
        if (!element.isJsonArray()) {
            return List.of();
        }
        JsonArray arr = element.getAsJsonArray();
        List<NpcTechniqueData> result = new ArrayList<>();
        for (JsonElement item : arr) {
            if (item == null || item.isJsonNull() || !item.isJsonObject()) {
                continue;
            }
            JsonObject tech = item.getAsJsonObject();
            result.add(new NpcTechniqueData(
                stringField(tech, "id", ""),
                stringField(tech, "display_name", ""),
                doubleField(tech, "proficiency", 0.0)
            ));
        }
        return result.isEmpty() ? List.of() : List.copyOf(result);
    }

    // ── trade_offers parsing ─────────────────────────────────────────────────

    static List<NpcTradeOffer> parseTradeOffers(JsonObject root) {
        if (!root.has("trade_offers") || root.get("trade_offers").isJsonNull()) {
            return List.of();
        }
        JsonElement element = root.get("trade_offers");
        if (!element.isJsonArray()) {
            return List.of();
        }
        JsonArray arr = element.getAsJsonArray();
        List<NpcTradeOffer> result = new ArrayList<>();
        for (JsonElement item : arr) {
            if (item == null || item.isJsonNull() || !item.isJsonObject()) {
                continue;
            }
            JsonObject offer = item.getAsJsonObject();
            result.add(new NpcTradeOffer(
                stringField(offer, "template_id", ""),
                stringField(offer, "display_name", ""),
                intField(offer, "count", 0),
                intField(offer, "price_bone_coins", 0)
            ));
        }
        return result.isEmpty() ? List.of() : List.copyOf(result);
    }

    // ── field helpers ────────────────────────────────────────────────────────

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
        String value = nullableString(root, fieldName);
        return value == null ? fallback : value;
    }

    private static String nullableString(JsonObject root, String fieldName) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return null;
        }
        try {
            return root.get(fieldName).getAsString();
        } catch (RuntimeException exception) {
            return null;
        }
    }

    private static double doubleField(JsonObject root, String fieldName, double fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            double value = root.get(fieldName).getAsDouble();
            return Double.isFinite(value) ? value : fallback;
        } catch (RuntimeException exception) {
            return fallback;
        }
    }
}
