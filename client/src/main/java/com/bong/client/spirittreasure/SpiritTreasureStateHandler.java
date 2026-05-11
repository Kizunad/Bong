package com.bong.client.spirittreasure;

import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

public final class SpiritTreasureStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return handle(envelope, System.currentTimeMillis());
    }

    ServerDataDispatch handle(ServerDataEnvelope envelope, long nowMs) {
        if (!"spirit_treasure_state".equals(envelope.type())) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring spirit treasure state: unsupported type '" + envelope.type() + "'"
            );
        }

        List<SpiritTreasureState> treasures = parseTreasures(envelope.payload());
        if (treasures == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring spirit_treasure_state payload: required fields missing or invalid"
            );
        }

        SpiritTreasureStateStore.replace(treasures, nowMs);
        return ServerDataDispatch.handled(
            envelope.type(),
            "spirit_treasure_state accepted (" + treasures.size() + " treasures)"
        );
    }

    private static List<SpiritTreasureState> parseTreasures(JsonObject payload) {
        JsonElement treasuresElement = payload.get("treasures");
        if (treasuresElement == null || !treasuresElement.isJsonArray()) {
            return null;
        }

        JsonArray treasuresArray = treasuresElement.getAsJsonArray();
        List<SpiritTreasureState> treasures = new ArrayList<>();
        for (JsonElement element : treasuresArray) {
            if (!element.isJsonObject()) {
                return null;
            }
            SpiritTreasureState parsed = parseTreasure(element.getAsJsonObject());
            if (parsed == null) {
                return null;
            }
            treasures.add(parsed);
        }
        return treasures;
    }

    private static SpiritTreasureState parseTreasure(JsonObject object) {
        String templateId = SpiritTreasureJson.readString(object, "template_id");
        String displayName = SpiritTreasureJson.readString(object, "display_name");
        Long instanceId = SpiritTreasureJson.readLong(object, "instance_id");
        Boolean equipped = SpiritTreasureJson.readBoolean(object, "equipped");
        Boolean passiveActive = SpiritTreasureJson.readBoolean(object, "passive_active");
        Double affinity = SpiritTreasureJson.readDouble(object, "affinity");
        Boolean sleeping = SpiritTreasureJson.readBoolean(object, "sleeping");
        String sourceSect = SpiritTreasureJson.readNullableString(object, "source_sect");
        String iconTexture = SpiritTreasureJson.readString(object, "icon_texture");
        List<SpiritTreasurePassive> passiveEffects = parsePassives(object);

        if (isBlank(templateId)
            || isBlank(displayName)
            || instanceId == null
            || instanceId < 0L
            || equipped == null
            || passiveActive == null
            || affinity == null
            || affinity < 0.0
            || affinity > 1.0
            || sleeping == null
            || sourceSect == null
            || isBlank(iconTexture)
            || passiveEffects == null) {
            return null;
        }

        return new SpiritTreasureState(
            templateId,
            displayName,
            instanceId,
            equipped,
            passiveActive,
            affinity,
            sleeping,
            sourceSect,
            iconTexture,
            passiveEffects
        );
    }

    private static List<SpiritTreasurePassive> parsePassives(JsonObject object) {
        JsonElement element = object.get("passive_effects");
        if (element == null || !element.isJsonArray()) {
            return null;
        }
        List<SpiritTreasurePassive> passives = new ArrayList<>();
        for (JsonElement passiveElement : element.getAsJsonArray()) {
            if (!passiveElement.isJsonObject()) {
                return null;
            }
            JsonObject passiveObject = passiveElement.getAsJsonObject();
            String kind = SpiritTreasureJson.readString(passiveObject, "kind");
            Double value = SpiritTreasureJson.readDouble(passiveObject, "value");
            String description = SpiritTreasureJson.readString(passiveObject, "description");
            if (isBlank(kind) || value == null || isBlank(description)) {
                return null;
            }
            passives.add(new SpiritTreasurePassive(kind, value, description));
        }
        return passives;
    }

    private static boolean isBlank(String value) {
        return value == null || value.isBlank();
    }
}
