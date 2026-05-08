package com.bong.client.network;

import com.bong.client.combat.SkillConfigStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.LinkedHashMap;
import java.util.Map;

/** Applies server-authoritative skill_config_snapshot payloads. */
public final class SkillConfigSnapshotHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonElement configsElement = envelope.payload().get("configs");
        if (configsElement == null || configsElement.isJsonNull() || !configsElement.isJsonObject()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring skill_config_snapshot payload: configs missing or not an object"
            );
        }

        Map<String, JsonObject> configs = new LinkedHashMap<>();
        for (Map.Entry<String, JsonElement> entry : configsElement.getAsJsonObject().entrySet()) {
            if (entry.getKey() == null || entry.getKey().isBlank()) {
                return ServerDataDispatch.noOp(
                    envelope.type(),
                    "Ignoring skill_config_snapshot payload: blank skill id"
                );
            }
            JsonElement value = entry.getValue();
            if (value == null || value.isJsonNull() || !value.isJsonObject()) {
                return ServerDataDispatch.noOp(
                    envelope.type(),
                    "Ignoring skill_config_snapshot payload: config for " + entry.getKey() + " is not an object"
                );
            }
            configs.put(entry.getKey(), value.getAsJsonObject().deepCopy());
        }

        SkillConfigStore.replace(configs);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied skill_config_snapshot (" + configs.size() + " configs)"
        );
    }
}
