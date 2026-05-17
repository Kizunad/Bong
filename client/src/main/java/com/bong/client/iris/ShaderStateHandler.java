package com.bong.client.iris;

import com.bong.client.BongClient;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

public final class ShaderStateHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "shader_state";

    private ShaderStateHandler() {
    }

    public static boolean handle(String jsonPayload) {
        if (jsonPayload == null || jsonPayload.isEmpty()) {
            return false;
        }
        try {
            JsonElement element = JsonParser.parseString(jsonPayload);
            if (!element.isJsonObject()) {
                return false;
            }
            JsonObject obj = element.getAsJsonObject();
            for (BongUniform uniform : BongUniform.values()) {
                JsonElement val = obj.get(uniform.shaderName());
                if (val != null && val.isJsonPrimitive() && val.getAsJsonPrimitive().isNumber()) {
                    BongShaderState.setTarget(uniform, val.getAsFloat());
                }
            }
            return true;
        } catch (Exception e) {
            BongClient.LOGGER.warn("[BongIris] Failed to parse shader_state payload: {}", e.getMessage());
            return false;
        }
    }
}
