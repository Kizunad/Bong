package com.bong.client.network;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.UUID;
import java.util.regex.Pattern;
import com.google.gson.JsonArray;

/**
 * `bong:vfx_event` CustomPayload 解析器。
 *
 * <p>与 {@link ServerDataEnvelope} 并列，差异：
 * <ul>
 *   <li>{@link ServerDataEnvelope} 的 payload 最终会流进 HUD state store，保留原始 {@code JsonObject}
 *       给下游 handler 再做类型分发</li>
 *   <li>本类的 payload 直接成型为强类型 {@link VfxEventPayload}，因为下游消费就是 "调一次 API"，
 *       再做 JSON walk 没意义</li>
 * </ul>
 *
 * <p>版本 + 大小限制照搬 {@link ServerDataEnvelope}，与 Rust 侧 {@code MAX_PAYLOAD_BYTES=8192}
 * 对齐。
 *
 * <p>字段校验对齐 {@code agent/packages/schema/src/vfx-event.ts} 的 TypeBox 约束：
 * <ul>
 *   <li>{@code target_player}: UUID 正则</li>
 *   <li>{@code anim_id}: Identifier 正则（namespace:path）</li>
 *   <li>{@code priority}: {@value VFX_ANIM_PRIORITY_MIN}..{@value VFX_ANIM_PRIORITY_MAX}</li>
 *   <li>{@code fade_in_ticks} / {@code fade_out_ticks}: 0..{@value VFX_FADE_TICKS_MAX}</li>
 * </ul>
 */
public final class VfxEventEnvelope {
    public static final int EXPECTED_VERSION = 1;
    public static final int MAX_PAYLOAD_BYTES = 8192;
    public static final int VFX_ANIM_PRIORITY_MIN = 100;
    public static final int VFX_ANIM_PRIORITY_MAX = 3999;
    public static final int VFX_FADE_TICKS_MAX = 40;
    public static final int VFX_PARTICLE_COUNT_MAX = 64;
    public static final int VFX_PARTICLE_DURATION_TICKS_MAX = 200;
    public static final int VFX_INLINE_ANIM_JSON_MAX_CHARS = 4096;

    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");
    private static final Pattern UUID_PATTERN =
        Pattern.compile("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$");
    private static final Pattern ANIM_ID_PATTERN = Pattern.compile("^[a-z0-9_]+:[a-z0-9_]+$");
    private static final Pattern COLOR_HEX_PATTERN = Pattern.compile("^#[0-9a-fA-F]{6}$");

    private VfxEventEnvelope() {
    }

    public static String decodeUtf8(byte[] bytes) {
        return new String(bytes, StandardCharsets.UTF_8);
    }

    public static VfxEventParseResult parse(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null) {
            return VfxEventParseResult.error("Malformed JSON: payload was null");
        }
        if (payloadSizeBytes < 0) {
            return VfxEventParseResult.error("Malformed JSON: payload byte size cannot be negative");
        }
        if (payloadSizeBytes > MAX_PAYLOAD_BYTES) {
            return VfxEventParseResult.error(
                "Payload exceeds max size of " + MAX_PAYLOAD_BYTES + " bytes: " + payloadSizeBytes
            );
        }

        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return VfxEventParseResult.error("Malformed JSON: expected top-level object");
            }
            JsonObject root = rootElement.getAsJsonObject();

            Integer version = readRequiredInteger(root, "v");
            if (version == null) {
                return VfxEventParseResult.error("Missing version 'v' field");
            }
            if (version != EXPECTED_VERSION) {
                return VfxEventParseResult.error("Unsupported version: " + version);
            }

            String type = readRequiredString(root, "type");
            if (type == null || type.isBlank()) {
                return VfxEventParseResult.error("Missing required field 'type'");
            }

            return switch (type) {
                case "play_anim" -> parsePlayAnim(root);
                case "play_anim_inline" -> parsePlayAnimInline(root);
                case "stop_anim" -> parseStopAnim(root);
                case "spawn_particle" -> parseSpawnParticle(root);
                default -> VfxEventParseResult.error("Unknown vfx_event type: '" + type + "'");
            };
        } catch (RuntimeException exception) {
            return VfxEventParseResult.error("Malformed JSON: " + exception.getMessage());
        }
    }

    private static VfxEventParseResult parsePlayAnim(JsonObject root) {
        UUID targetPlayer = parseRequiredUuid(root, "target_player");
        if (targetPlayer == null) {
            return VfxEventParseResult.error("Invalid or missing 'target_player' UUID");
        }
        Identifier animId = parseRequiredAnimId(root, "anim_id");
        if (animId == null) {
            return VfxEventParseResult.error("Invalid or missing 'anim_id'");
        }
        Integer priority = readRequiredInteger(root, "priority");
        if (priority == null) {
            return VfxEventParseResult.error("Missing required field 'priority'");
        }
        if (priority < VFX_ANIM_PRIORITY_MIN || priority > VFX_ANIM_PRIORITY_MAX) {
            return VfxEventParseResult.error(
                "Field 'priority' out of range [" + VFX_ANIM_PRIORITY_MIN + "," + VFX_ANIM_PRIORITY_MAX + "]: " + priority
            );
        }
        OptionalInt fadeInTicks = readOptionalFadeTicks(root, "fade_in_ticks");
        if (fadeInTicks == null) {
            return VfxEventParseResult.error(
                "Field 'fade_in_ticks' out of range [0," + VFX_FADE_TICKS_MAX + "]"
            );
        }
        return VfxEventParseResult.success(
            new VfxEventPayload.PlayAnim(targetPlayer, animId, priority, fadeInTicks)
        );
    }

    private static VfxEventParseResult parsePlayAnimInline(JsonObject root) {
        UUID targetPlayer = parseRequiredUuid(root, "target_player");
        if (targetPlayer == null) {
            return VfxEventParseResult.error("Invalid or missing 'target_player' UUID");
        }
        Identifier animId = parseRequiredAnimId(root, "anim_id");
        if (animId == null) {
            return VfxEventParseResult.error("Invalid or missing 'anim_id'");
        }
        String animJson = readRequiredString(root, "anim_json");
        if (animJson == null || animJson.isEmpty()) {
            return VfxEventParseResult.error("Invalid or missing 'anim_json'");
        }
        if (animJson.length() > VFX_INLINE_ANIM_JSON_MAX_CHARS) {
            return VfxEventParseResult.error(
                "Field 'anim_json' exceeds max length of " + VFX_INLINE_ANIM_JSON_MAX_CHARS
            );
        }
        Integer priority = readRequiredInteger(root, "priority");
        if (priority == null) {
            return VfxEventParseResult.error("Missing required field 'priority'");
        }
        if (priority < VFX_ANIM_PRIORITY_MIN || priority > VFX_ANIM_PRIORITY_MAX) {
            return VfxEventParseResult.error(
                "Field 'priority' out of range [" + VFX_ANIM_PRIORITY_MIN + "," + VFX_ANIM_PRIORITY_MAX + "]: " + priority
            );
        }
        OptionalInt fadeInTicks = readOptionalFadeTicks(root, "fade_in_ticks");
        if (fadeInTicks == null) {
            return VfxEventParseResult.error(
                "Field 'fade_in_ticks' out of range [0," + VFX_FADE_TICKS_MAX + "]"
            );
        }
        return VfxEventParseResult.success(
            new VfxEventPayload.PlayAnimInline(targetPlayer, animId, animJson, priority, fadeInTicks)
        );
    }

    private static VfxEventParseResult parseStopAnim(JsonObject root) {
        UUID targetPlayer = parseRequiredUuid(root, "target_player");
        if (targetPlayer == null) {
            return VfxEventParseResult.error("Invalid or missing 'target_player' UUID");
        }
        Identifier animId = parseRequiredAnimId(root, "anim_id");
        if (animId == null) {
            return VfxEventParseResult.error("Invalid or missing 'anim_id'");
        }
        OptionalInt fadeOutTicks = readOptionalFadeTicks(root, "fade_out_ticks");
        if (fadeOutTicks == null) {
            return VfxEventParseResult.error(
                "Field 'fade_out_ticks' out of range [0," + VFX_FADE_TICKS_MAX + "]"
            );
        }
        return VfxEventParseResult.success(
            new VfxEventPayload.StopAnim(targetPlayer, animId, fadeOutTicks)
        );
    }

    private static VfxEventParseResult parseSpawnParticle(JsonObject root) {
        Identifier eventId = parseRequiredAnimId(root, "event_id");
        if (eventId == null) {
            return VfxEventParseResult.error("Invalid or missing 'event_id'");
        }
        double[] origin = parseRequiredVec3(root, "origin");
        if (origin == null) {
            return VfxEventParseResult.error("Invalid or missing 'origin' vec3");
        }

        Optional<double[]> direction;
        JsonElement dirElem = root.get("direction");
        if (dirElem == null || dirElem.isJsonNull()) {
            direction = Optional.empty();
        } else {
            double[] dir = parseVec3Element(dirElem);
            if (dir == null) {
                return VfxEventParseResult.error("Invalid 'direction' vec3");
            }
            direction = Optional.of(dir);
        }

        OptionalInt colorRgb;
        JsonElement colorElem = root.get("color");
        if (colorElem == null || colorElem.isJsonNull()) {
            colorRgb = OptionalInt.empty();
        } else {
            if (!colorElem.isJsonPrimitive() || !colorElem.getAsJsonPrimitive().isString()) {
                return VfxEventParseResult.error("Field 'color' must be a string");
            }
            String raw = colorElem.getAsString();
            if (!COLOR_HEX_PATTERN.matcher(raw).matches()) {
                return VfxEventParseResult.error("Field 'color' must match #RRGGBB");
            }
            colorRgb = OptionalInt.of(Integer.parseInt(raw.substring(1), 16));
        }

        Optional<Double> strength;
        JsonElement strengthElem = root.get("strength");
        if (strengthElem == null || strengthElem.isJsonNull()) {
            strength = Optional.empty();
        } else {
            if (!strengthElem.isJsonPrimitive() || !strengthElem.getAsJsonPrimitive().isNumber()) {
                return VfxEventParseResult.error("Field 'strength' must be a number");
            }
            double value = strengthElem.getAsDouble();
            if (!Double.isFinite(value) || value < 0.0 || value > 1.0) {
                return VfxEventParseResult.error("Field 'strength' out of range [0.0, 1.0]");
            }
            strength = Optional.of(value);
        }

        OptionalInt count = readOptionalBoundedInteger(
            root, "count", 1, VFX_PARTICLE_COUNT_MAX);
        if (count == null) {
            return VfxEventParseResult.error(
                "Field 'count' out of range [1," + VFX_PARTICLE_COUNT_MAX + "]");
        }
        OptionalInt durationTicks = readOptionalBoundedInteger(
            root, "duration_ticks", 1, VFX_PARTICLE_DURATION_TICKS_MAX);
        if (durationTicks == null) {
            return VfxEventParseResult.error(
                "Field 'duration_ticks' out of range [1," + VFX_PARTICLE_DURATION_TICKS_MAX + "]");
        }

        return VfxEventParseResult.success(new VfxEventPayload.SpawnParticle(
            eventId, origin, direction, colorRgb, strength, count, durationTicks));
    }

    /**
     * 解析 {@code [x, y, z]} 数组，3 个 finite number。返回 null 表示形态不对。
     * 抽出来是因为 origin/direction 共用，且客户端要确保不把 NaN/Infinity 传给渲染层。
     */
    private static double[] parseRequiredVec3(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return null;
        }
        return parseVec3Element(element);
    }

    private static double[] parseVec3Element(JsonElement element) {
        if (!element.isJsonArray()) {
            return null;
        }
        JsonArray arr = element.getAsJsonArray();
        if (arr.size() != 3) {
            return null;
        }
        double[] out = new double[3];
        for (int i = 0; i < 3; i++) {
            JsonElement e = arr.get(i);
            if (!e.isJsonPrimitive() || !e.getAsJsonPrimitive().isNumber()) {
                return null;
            }
            double v = e.getAsDouble();
            if (!Double.isFinite(v)) {
                return null;
            }
            out[i] = v;
        }
        return out;
    }

    /**
     * 可选整数字段，范围校验。
     * 语义：
     * <ul>
     *   <li>字段缺失 → {@link OptionalInt#empty()}</li>
     *   <li>值在 [min, max] → {@link OptionalInt#of(int)}</li>
     *   <li>值越界 → {@code null}（调用方应报错）</li>
     * </ul>
     * 类型错误仍走异常（与 readRequiredInteger 一致）。
     */
    private static OptionalInt readOptionalBoundedInteger(
        JsonObject root, String fieldName, int min, int max) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return OptionalInt.empty();
        }
        JsonPrimitive primitive = requirePrimitive(fieldName, element);
        if (!primitive.isNumber()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a number");
        }
        String raw = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) {
            throw new IllegalStateException("field '" + fieldName + "' must be an integer");
        }
        int value = Integer.parseInt(raw);
        if (value < min || value > max) {
            return null;
        }
        return OptionalInt.of(value);
    }

    private static UUID parseRequiredUuid(JsonObject root, String fieldName) {
        String raw = readRequiredString(root, fieldName);
        if (raw == null || !UUID_PATTERN.matcher(raw).matches()) {
            return null;
        }
        try {
            return UUID.fromString(raw);
        } catch (IllegalArgumentException ignored) {
            return null;
        }
    }

    private static Identifier parseRequiredAnimId(JsonObject root, String fieldName) {
        String raw = readRequiredString(root, fieldName);
        if (raw == null) {
            return null;
        }
        // 先用我们的正则过滤（schema 约束），再由 Identifier 做最终解析；
        // Identifier 允许更多字符（如 /），但 Bong 的约定是 `ns:path` 全小写。
        if (!ANIM_ID_PATTERN.matcher(raw).matches()) {
            return null;
        }
        try {
            return new Identifier(raw);
        } catch (RuntimeException ignored) {
            return null;
        }
    }

    /**
     * 缺省返回 {@link OptionalInt#empty()}；值越界时返回 null 以示错误，由调用方转成明确
     * 错误信息（区别于 "没提供" 的合法 empty）。
     */
    private static OptionalInt readOptionalFadeTicks(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return OptionalInt.empty();
        }
        JsonPrimitive primitive = requirePrimitive(fieldName, element);
        if (!primitive.isNumber()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a number");
        }
        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            throw new IllegalStateException("field '" + fieldName + "' must be an integer");
        }
        int value = Integer.parseInt(rawValue);
        if (value < 0 || value > VFX_FADE_TICKS_MAX) {
            return null;
        }
        return OptionalInt.of(value);
    }

    private static Integer readRequiredInteger(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return null;
        }
        JsonPrimitive primitive = requirePrimitive(fieldName, element);
        if (!primitive.isNumber()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a number");
        }
        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            throw new IllegalStateException("field '" + fieldName + "' must be an integer");
        }
        return Integer.parseInt(rawValue);
    }

    private static String readRequiredString(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return null;
        }
        JsonPrimitive primitive = requirePrimitive(fieldName, element);
        if (!primitive.isString()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a string");
        }
        return primitive.getAsString();
    }

    private static JsonPrimitive requirePrimitive(String fieldName, JsonElement element) {
        if (!element.isJsonPrimitive()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a primitive value");
        }
        return element.getAsJsonPrimitive();
    }
}
