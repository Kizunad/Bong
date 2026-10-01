package com.bong.client.botany;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.fabricmc.fabric.api.resource.ResourceManagerHelper;
import net.fabricmc.fabric.api.resource.SimpleSynchronousResourceReloadListener;
import net.minecraft.resource.Resource;
import net.minecraft.resource.ResourceManager;
import net.minecraft.resource.ResourceType;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.InputStreamReader;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;

/** 客户端读取 shared/botany/plants.json 后提供植物模型元数据。 */
public final class PlantModelRegistry implements SimpleSynchronousResourceReloadListener {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-client-botany");
    private static final Identifier CATALOG_ID = new Identifier("bong-client", "botany/plants.json");
    private static volatile Map<String, PlantDefinition> definitions = Map.of();

    private PlantModelRegistry() {
    }

    public static void register() {
        ResourceManagerHelper.get(ResourceType.CLIENT_RESOURCES)
            .registerReloadListener(new PlantModelRegistry());
    }

    public static Optional<PlantStageModel> stage(String plantId, PlantGrowthStage stage) {
        PlantDefinition definition = definitions.get(normalize(plantId));
        if (definition == null) {
            return Optional.empty();
        }
        PlantStageModel model = definition.stages.get(stage == null ? PlantGrowthStage.MATURE : stage);
        if (model == null && stage != PlantGrowthStage.MATURE) {
            model = definition.stages.get(PlantGrowthStage.MATURE);
        }
        return Optional.ofNullable(model);
    }

    public static OptionalIntValue tint(String plantId) {
        PlantDefinition definition = definitions.get(normalize(plantId));
        return definition == null ? OptionalIntValue.empty() : OptionalIntValue.of(definition.tintRgb);
    }

    public static List<String> geoPlantIds() {
        return definitions.values().stream()
            .filter(definition -> definition.stages.get(PlantGrowthStage.MATURE).isGeo())
            .map(PlantDefinition::id).distinct().sorted().toList();
    }

    public static String displayName(String plantId) {
        PlantDefinition definition = definitions.get(normalize(plantId));
        return definition == null ? plantId : definition.name();
    }

    static void loadForTest(String json) {
        definitions = parse(json);
    }

    static int definitionCount() {
        return definitions.size();
    }

    @Override
    public Identifier getFabricId() {
        return new Identifier("bong-client", "botany_models");
    }

    @Override
    public void reload(ResourceManager manager) {
        PlantGeoRenderer.clearCache();
        Optional<Resource> resource = manager.getResource(CATALOG_ID);
        if (resource.isEmpty()) {
            definitions = Map.of();
            LOGGER.warn("Shared plant catalog is missing: {}", CATALOG_ID);
            return;
        }
        try (Reader reader = new InputStreamReader(resource.get().getInputStream(), StandardCharsets.UTF_8)) {
            definitions = parse(reader);
            LOGGER.info("Loaded {} plant model definitions", definitions.size());
        } catch (Exception error) {
            definitions = Map.of();
            LOGGER.error("Failed to load shared plant catalog {}", CATALOG_ID, error);
        }
    }

    private static Map<String, PlantDefinition> parse(String json) {
        try {
            return parse(JsonParser.parseString(json).getAsJsonObject());
        } catch (RuntimeException error) {
            LOGGER.error("Invalid shared plant catalog JSON", error);
            return Map.of();
        }
    }

    private static Map<String, PlantDefinition> parse(Reader reader) {
        try {
            return parse(JsonParser.parseReader(reader).getAsJsonObject());
        } catch (RuntimeException error) {
            LOGGER.error("Invalid shared plant catalog JSON", error);
            return Map.of();
        }
    }

    private static Map<String, PlantDefinition> parse(JsonObject root) {
        JsonArray plants = root.getAsJsonArray("plants");
        if (plants == null) {
            return Map.of();
        }
        Map<String, PlantDefinition> parsed = new HashMap<>();
        for (JsonElement element : plants) {
            if (!element.isJsonObject()) {
                continue;
            }
            JsonObject plant = element.getAsJsonObject();
            String id = stringValue(plant, "id");
            JsonObject visual = objectValue(plant, "visual");
            JsonObject stages = visual == null ? null : objectValue(visual, "stages");
            if (id == null || visual == null || stages == null) {
                continue;
            }
            Map<PlantGrowthStage, PlantStageModel> stageModels = new HashMap<>();
            for (Map.Entry<String, JsonElement> entry : stages.entrySet()) {
                PlantGrowthStage stage = PlantGrowthStage.fromWireName(entry.getKey());
                PlantStageModel model = parseStage(entry.getValue());
                if (model != null) {
                    stageModels.put(stage, model);
                }
            }
            if (!stageModels.containsKey(PlantGrowthStage.MATURE)) {
                continue;
            }
            int tint = integerValue(visual, "tint_rgb", 0xFFFFFF) & 0xFFFFFF;
            String name = stringValue(plant, "name");
            PlantDefinition definition = new PlantDefinition(id, name == null ? id : name, tint, Map.copyOf(stageModels));
            parsed.put(normalize(id), definition);
            JsonArray aliases = plant.getAsJsonArray("aliases");
            if (aliases != null) {
                for (JsonElement alias : aliases) {
                    if (alias.isJsonPrimitive() && alias.getAsJsonPrimitive().isString()) {
                        parsed.put(normalize(alias.getAsString()), definition);
                    }
                }
            }
        }
        return Map.copyOf(parsed);
    }

    private static PlantStageModel parseStage(JsonElement element) {
        if (element == null || !element.isJsonObject()) {
            return null;
        }
        JsonObject stage = element.getAsJsonObject();
        String kind = stringValue(stage, "kind");
        if (!"billboard".equals(kind) && !"geo".equals(kind)) {
            return null;
        }
        String texture = stringValue(stage, "texture");
        if (texture == null) {
            return null;
        }
        Identifier textureId = Identifier.tryParse(texture);
        if (textureId == null) {
            return null;
        }
        float scale = stage.has("scale") && stage.get("scale").isJsonPrimitive()
            ? stage.get("scale").getAsFloat()
            : 1.0f;
        float[] offset = new float[] { 0.0f, 0.0f, 0.0f };
        JsonArray rawOffset = stage.getAsJsonArray("offset");
        if (rawOffset != null && rawOffset.size() == 3) {
            for (int index = 0; index < 3; index++) {
                offset[index] = rawOffset.get(index).getAsFloat();
            }
        }
        if (!Float.isFinite(scale) || scale <= 0
            || !Float.isFinite(offset[0]) || !Float.isFinite(offset[1]) || !Float.isFinite(offset[2])) {
            return null;
        }
        Identifier geometry = null;
        PlantAnimation animation = null;
        if ("geo".equals(kind)) {
            String path = stringValue(stage, "geometry");
            geometry = path == null ? null : Identifier.tryParse(path);
            if (geometry == null || !geometry.getPath().endsWith(".geo.json")) {
                return null;
            }
            JsonObject clip = objectValue(stage, "animation");
            if (clip != null) {
                String resource = stringValue(clip, "resource");
                String idle = stringValue(clip, "idle");
                Identifier resourceId = resource == null ? null : Identifier.tryParse(resource);
                if (resourceId == null || !resourceId.getPath().endsWith(".animation.json")
                    || idle == null || idle.isBlank()) {
                    return null;
                }
                animation = new PlantAnimation(resourceId, idle);
            }
        }
        return new PlantStageModel(textureId, scale, offset, geometry, animation);
    }

    private static JsonObject objectValue(JsonObject object, String field) {
        JsonElement value = object.get(field);
        return value != null && value.isJsonObject() ? value.getAsJsonObject() : null;
    }

    private static String stringValue(JsonObject object, String field) {
        JsonElement value = object.get(field);
        return value != null && value.isJsonPrimitive() && value.getAsJsonPrimitive().isString()
            ? value.getAsString()
            : null;
    }

    private static int integerValue(JsonObject object, String field, int fallback) {
        JsonElement value = object.get(field);
        return value != null && value.isJsonPrimitive() && value.getAsJsonPrimitive().isNumber()
            ? value.getAsInt()
            : fallback;
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim().toLowerCase(Locale.ROOT);
    }

    public record PlantAnimation(Identifier resource, String idle) {
    }

    public record PlantStageModel(Identifier texture, float scale, float[] offset,
                                  Identifier geometry, PlantAnimation animation) {
        public PlantStageModel {
            offset = offset == null || offset.length != 3 ? new float[] { 0.0f, 0.0f, 0.0f } : offset.clone();
        }

        public float offsetX() {
            return offset[0];
        }

        public float offsetY() {
            return offset[1];
        }

        public float offsetZ() {
            return offset[2];
        }

        public boolean isGeo() {
            return geometry != null;
        }
    }

    public record OptionalIntValue(boolean present, int value) {
        static OptionalIntValue empty() {
            return new OptionalIntValue(false, 0);
        }

        static OptionalIntValue of(int value) {
            return new OptionalIntValue(true, value);
        }
    }

    private record PlantDefinition(String id, String name, int tintRgb, Map<PlantGrowthStage, PlantStageModel> stages) {
    }
}
