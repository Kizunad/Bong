package com.bong.client.network;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioLoopConfig;
import com.bong.client.audio.AudioPosition;
import com.bong.client.audio.AudioRecipe;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.regex.Pattern;

public final class AudioEventEnvelope {
    public static final int EXPECTED_VERSION = 1;
    public static final int MAX_PAYLOAD_BYTES = 8192;
    public static final float AUDIO_VOLUME_MAX = 4.0f;
    public static final float AUDIO_PITCH_MIN = 0.1f;
    public static final float AUDIO_PITCH_MAX = 2.0f;
    public static final int AUDIO_PRIORITY_MAX = 100;

    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");
    private static final Pattern RECIPE_ID_PATTERN = Pattern.compile("^[a-z0-9_]+$");
    private static final Pattern IDENTIFIER_PATTERN = Pattern.compile("^[a-z0-9_.-]+:[a-z0-9_./-]+$");

    private AudioEventEnvelope() {
    }

    public static String decodeUtf8(byte[] bytes) {
        return new String(bytes, StandardCharsets.UTF_8);
    }

    public static AudioEventParseResult parsePlay(String jsonPayload, int payloadSizeBytes) {
        JsonObject root = parseRoot(jsonPayload, payloadSizeBytes);
        if (root == null) {
            return AudioEventParseResult.error("Malformed JSON: expected top-level object");
        }
        Integer version = readRequiredInteger(root, "v");
        if (version == null) {
            return AudioEventParseResult.error("Missing version 'v' field");
        }
        if (version != EXPECTED_VERSION) {
            return AudioEventParseResult.error("Unsupported version: " + version);
        }
        String recipeId = readRequiredString(root, "recipe_id");
        if (!isRecipeId(recipeId)) {
            return AudioEventParseResult.error("Invalid or missing 'recipe_id'");
        }
        Long instanceId = readRequiredLong(root, "instance_id");
        if (instanceId == null || instanceId < 0) {
            return AudioEventParseResult.error("Invalid or missing 'instance_id'");
        }
        Optional<AudioPosition> pos = readOptionalPos(root, "pos");
        if (pos == null) {
            return AudioEventParseResult.error("Invalid 'pos' vec3i");
        }
        Optional<String> flag = readOptionalNonBlankString(root, "flag");
        if (flag == null) {
            return AudioEventParseResult.error("Invalid 'flag'");
        }
        Float volumeMul = readRequiredFloat(root, "volume_mul");
        if (volumeMul == null || !Float.isFinite(volumeMul) || volumeMul < 0.0f || volumeMul > AUDIO_VOLUME_MAX) {
            return AudioEventParseResult.error("Field 'volume_mul' out of range [0," + AUDIO_VOLUME_MAX + "]");
        }
        Float pitchShift = readRequiredFloat(root, "pitch_shift");
        if (pitchShift == null || !Float.isFinite(pitchShift) || pitchShift < -1.0f || pitchShift > 1.0f) {
            return AudioEventParseResult.error("Field 'pitch_shift' out of range [-1,1]");
        }
        AudioRecipe recipe = parseRecipe(root.get("recipe"));
        if (recipe == null) {
            return AudioEventParseResult.error("Invalid or missing 'recipe'");
        }
        if (!recipeId.equals(recipe.id())) {
            return AudioEventParseResult.error("Field 'recipe_id' must equal recipe.id");
        }
        return AudioEventParseResult.success(new AudioEventPayload.PlaySoundRecipe(
            recipeId,
            instanceId,
            pos,
            flag,
            volumeMul,
            pitchShift,
            recipe
        ));
    }

    public static AudioEventParseResult parseStop(String jsonPayload, int payloadSizeBytes) {
        JsonObject root = parseRoot(jsonPayload, payloadSizeBytes);
        if (root == null) {
            return AudioEventParseResult.error("Malformed JSON: expected top-level object");
        }
        Integer version = readRequiredInteger(root, "v");
        if (version == null) {
            return AudioEventParseResult.error("Missing version 'v' field");
        }
        if (version != EXPECTED_VERSION) {
            return AudioEventParseResult.error("Unsupported version: " + version);
        }
        Long instanceId = readRequiredLong(root, "instance_id");
        if (instanceId == null || instanceId <= 0) {
            return AudioEventParseResult.error("Invalid or missing 'instance_id'");
        }
        Integer fadeOutTicks = readRequiredInteger(root, "fade_out_ticks");
        if (fadeOutTicks == null || fadeOutTicks < 0) {
            return AudioEventParseResult.error("Invalid or missing 'fade_out_ticks'");
        }
        return AudioEventParseResult.success(new AudioEventPayload.StopSoundRecipe(instanceId, fadeOutTicks));
    }

    static JsonObject parseRoot(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null || payloadSizeBytes < 0 || payloadSizeBytes > MAX_PAYLOAD_BYTES) {
            return null;
        }
        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            return rootElement.isJsonObject() ? rootElement.getAsJsonObject() : null;
        } catch (RuntimeException ignored) {
            return null;
        }
    }

    static AudioRecipe parseRecipe(JsonElement element) {
        if (element == null || !element.isJsonObject()) {
            return null;
        }
        JsonObject root = element.getAsJsonObject();
        String id = readRequiredString(root, "id");
        if (!isRecipeId(id)) {
            return null;
        }
        JsonElement layersElement = root.get("layers");
        if (layersElement == null || !layersElement.isJsonArray()) {
            return null;
        }
        List<AudioLayer> layers = new ArrayList<>();
        for (JsonElement layerElement : layersElement.getAsJsonArray()) {
            AudioLayer layer = parseLayer(layerElement);
            if (layer == null) {
                return null;
            }
            layers.add(layer);
        }
        if (layers.isEmpty() || layers.size() > 8) {
            return null;
        }
        Optional<AudioLoopConfig> loop = parseLoop(root.get("loop"));
        if (loop == null) {
            return null;
        }
        Integer priority = readRequiredInteger(root, "priority");
        if (priority == null || priority < 0 || priority > AUDIO_PRIORITY_MAX) {
            return null;
        }
        AudioAttenuation attenuation = AudioAttenuation.fromWire(readRequiredString(root, "attenuation"));
        AudioCategory category = AudioCategory.fromWire(readRequiredString(root, "category"));
        if (attenuation == null || category == null) {
            return null;
        }
        AudioBus bus = AudioBus.fromWire(readOptionalString(root, "bus"));
        return new AudioRecipe(id, layers, loop, priority, attenuation, category, bus);
    }

    private static AudioLayer parseLayer(JsonElement element) {
        if (element == null || !element.isJsonObject()) {
            return null;
        }
        JsonObject root = element.getAsJsonObject();
        Identifier sound = parseIdentifier(readRequiredString(root, "sound"));
        Float volume = readRequiredFloat(root, "volume");
        Float pitch = readRequiredFloat(root, "pitch");
        Integer delayTicks = readRequiredInteger(root, "delay_ticks");
        if (sound == null || volume == null || pitch == null || delayTicks == null) {
            return null;
        }
        if (!Float.isFinite(volume) || volume < 0.0f || volume > AUDIO_VOLUME_MAX) {
            return null;
        }
        if (!Float.isFinite(pitch) || pitch < AUDIO_PITCH_MIN || pitch > AUDIO_PITCH_MAX) {
            return null;
        }
        if (delayTicks < 0) {
            return null;
        }
        return new AudioLayer(sound, volume, pitch, delayTicks);
    }

    private static Optional<AudioLoopConfig> parseLoop(JsonElement element) {
        if (element == null || element.isJsonNull()) {
            return Optional.empty();
        }
        if (!element.isJsonObject()) {
            return null;
        }
        JsonObject root = element.getAsJsonObject();
        Integer intervalTicks = readRequiredInteger(root, "interval_ticks");
        String whileFlag = readRequiredString(root, "while_flag");
        if (intervalTicks == null || intervalTicks <= 0 || whileFlag == null || whileFlag.isBlank()) {
            return null;
        }
        return Optional.of(new AudioLoopConfig(intervalTicks, whileFlag));
    }

    static Optional<AudioPosition> readOptionalPos(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return Optional.empty();
        }
        if (!element.isJsonArray()) {
            return null;
        }
        JsonArray array = element.getAsJsonArray();
        if (array.size() != 3) {
            return null;
        }
        Integer x = readIntegerElement(array.get(0));
        Integer y = readIntegerElement(array.get(1));
        Integer z = readIntegerElement(array.get(2));
        if (x == null || y == null || z == null) {
            return null;
        }
        return Optional.of(new AudioPosition(x, y, z));
    }

    private static Optional<String> readOptionalNonBlankString(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return Optional.empty();
        }
        if (!element.isJsonPrimitive() || !element.getAsJsonPrimitive().isString()) {
            return null;
        }
        String value = element.getAsString();
        return value.isBlank() ? null : Optional.of(value);
    }

    private static String readOptionalString(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return null;
        }
        if (!element.isJsonPrimitive() || !element.getAsJsonPrimitive().isString()) {
            return null;
        }
        return element.getAsString();
    }

    private static Identifier parseIdentifier(String raw) {
        if (raw == null || !IDENTIFIER_PATTERN.matcher(raw).matches()) {
            return null;
        }
        try {
            return new Identifier(raw);
        } catch (RuntimeException ignored) {
            return null;
        }
    }

    private static boolean isRecipeId(String raw) {
        return raw != null && RECIPE_ID_PATTERN.matcher(raw).matches();
    }

    static Integer readRequiredInteger(JsonObject root, String fieldName) {
        return readIntegerElement(root.get(fieldName));
    }

    private static Long readRequiredLong(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        String raw = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) {
            return null;
        }
        try {
            return Long.parseLong(raw);
        } catch (NumberFormatException ignored) {
            return null;
        }
    }

    private static Integer readIntegerElement(JsonElement element) {
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        String raw = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) {
            return null;
        }
        try {
            return Integer.parseInt(raw);
        } catch (NumberFormatException ignored) {
            return null;
        }
    }

    static Float readRequiredFloat(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        return primitive.getAsFloat();
    }

    static String readRequiredString(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }
}
