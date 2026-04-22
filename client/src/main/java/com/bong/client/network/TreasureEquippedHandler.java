package com.bong.client.network;

import com.bong.client.combat.EquippedTreasure;
import com.bong.client.combat.TreasurePanelSync;
import com.bong.client.combat.TreasureEquippedStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

public final class TreasureEquippedHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String slot = readString(payload, "slot", "treasure_belt_0");

        JsonElement treasureElem = payload.get("treasure");
        if (treasureElem == null || treasureElem.isJsonNull() || !treasureElem.isJsonObject()) {
            TreasureEquippedStore.putOrClear(slot, null);
            TreasurePanelSync.syncFromStore();
            return ServerDataDispatch.handled(envelope.type(), "Cleared treasure slot " + slot);
        }

        JsonObject t = treasureElem.getAsJsonObject();
        long instanceId = t.get("instance_id").getAsLong();
        String templateId = t.get("template_id").getAsString();
        String displayName = t.get("display_name").getAsString();

        TreasureEquippedStore.putOrClear(slot, new EquippedTreasure(slot, instanceId, templateId, displayName));
        TreasurePanelSync.syncFromStore();
        return ServerDataDispatch.handled(
            envelope.type(),
            "Equipped treasure " + templateId + " to " + slot
        );
    }

    private static String readString(JsonObject obj, String field, String fallback) {
        JsonElement e = obj.get(field);
        if (e == null || !e.isJsonPrimitive()) return fallback;
        return e.getAsString();
    }
}
