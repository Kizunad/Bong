package com.bong.client.network;

import com.bong.client.botany.BotanySkillStore;
import com.bong.client.botany.BotanySkillViewModel;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.regex.Pattern;

public final class BotanySkillHandler implements ServerDataHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Integer level = readOptionalInteger(payload, "level");
        Long xp = readOptionalLong(payload, "xp");
        Long xpToNextLevel = readOptionalLong(payload, "xp_to_next_level");
        Integer autoUnlockLevel = readOptionalInteger(payload, "auto_unlock_level");
        if (level == null || xp == null || xpToNextLevel == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring botany_skill payload: required fields 'level', 'xp', or 'xp_to_next_level' are missing or invalid"
            );
        }

        BotanySkillViewModel snapshot = BotanySkillViewModel.create(
            level,
            xp,
            xpToNextLevel,
            autoUnlockLevel == null ? 3 : autoUnlockLevel
        );
        BotanySkillStore.replace(snapshot);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied botany_skill level " + snapshot.level() + " to BotanySkillStore"
        );
    }

    private static Integer readOptionalInteger(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            return null;
        }
        return Integer.parseInt(rawValue);
    }

    private static Long readOptionalLong(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            return null;
        }
        return Long.parseLong(rawValue);
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }
}
