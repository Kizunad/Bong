package com.bong.client.network;

import com.bong.client.botany.BotanyPlantRenderProfile;
import com.bong.client.botany.BotanyPlantRenderProfileStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.util.ArrayList;
import java.util.List;

public final class BotanyPlantRenderProfileHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonElement profilesElement = envelope.payload().get("profiles");
        if (profilesElement == null || !profilesElement.isJsonArray()) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring botany render profiles: profiles[] missing");
        }
        List<BotanyPlantRenderProfile> profiles = new ArrayList<>();
        JsonArray array = profilesElement.getAsJsonArray();
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) {
                continue;
            }
            JsonObject object = element.getAsJsonObject();
            String plantId = readString(object, "plant_id");
            String baseMeshRef = readString(object, "base_mesh_ref");
            Integer tintRgb = readInt(object, "tint_rgb");
            if (plantId == null || plantId.isBlank() || baseMeshRef == null || baseMeshRef.isBlank() || tintRgb == null) {
                continue;
            }
            profiles.add(new BotanyPlantRenderProfile(
                plantId,
                baseMeshRef,
                tintRgb,
                readInt(object, "tint_rgb_secondary"),
                BotanyPlantRenderProfile.ModelOverlay.fromWireName(readString(object, "model_overlay"))
            ));
        }
        BotanyPlantRenderProfileStore.replaceAll(profiles);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied " + profiles.size() + " botany v2 render profile(s)"
        );
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isString() ? primitive.getAsString() : null;
    }

    private static Integer readInt(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsInt() : null;
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }
}
