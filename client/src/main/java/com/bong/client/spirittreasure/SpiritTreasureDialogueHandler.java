package com.bong.client.spirittreasure;

import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

public final class SpiritTreasureDialogueHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return handle(envelope, System.currentTimeMillis());
    }

    ServerDataDispatch handle(ServerDataEnvelope envelope, long nowMs) {
        if (!"spirit_treasure_dialogue".equals(envelope.type())) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring spirit treasure dialogue: unsupported type '" + envelope.type() + "'"
            );
        }

        SpiritTreasureDialogue dialogue = parse(envelope.payload(), nowMs);
        if (dialogue == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring spirit_treasure_dialogue payload: required fields missing or invalid"
            );
        }

        SpiritTreasureDialogueStore.append(dialogue);
        return ServerDataDispatch.handled(
            envelope.type(),
            "spirit_treasure_dialogue accepted (" + dialogue.treasureId() + ")"
        );
    }

    private static SpiritTreasureDialogue parse(JsonObject payload, long nowMs) {
        JsonElement dialogueElement = payload.get("dialogue");
        if (dialogueElement == null || !dialogueElement.isJsonObject()) {
            return null;
        }

        JsonObject dialogue = dialogueElement.getAsJsonObject();
        String requestId = SpiritTreasureJson.readString(dialogue, "request_id");
        String characterId = SpiritTreasureJson.readString(dialogue, "character_id");
        String treasureId = SpiritTreasureJson.readString(dialogue, "treasure_id");
        String text = SpiritTreasureJson.readString(dialogue, "text");
        String tone = SpiritTreasureJson.readString(dialogue, "tone");
        Double affinityDelta = SpiritTreasureJson.readDouble(dialogue, "affinity_delta");
        String displayName = SpiritTreasureJson.readString(payload, "display_name");
        String zone = SpiritTreasureJson.readString(payload, "zone");

        if (isBlank(requestId)
            || isBlank(characterId)
            || isBlank(treasureId)
            || isBlank(text)
            || isBlank(tone)
            || affinityDelta == null
            || affinityDelta < -1.0
            || affinityDelta > 1.0
            || isBlank(displayName)
            || isBlank(zone)) {
            return null;
        }

        return new SpiritTreasureDialogue(
            requestId,
            characterId,
            treasureId,
            displayName,
            text,
            tone,
            affinityDelta,
            zone,
            Math.max(0L, nowMs)
        );
    }

    private static boolean isBlank(String value) {
        return value == null || value.isBlank();
    }
}
