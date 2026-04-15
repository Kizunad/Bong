package com.bong.client.network;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;
import java.util.OptionalInt;
import java.util.UUID;
import java.util.regex.Pattern;

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
 * <p>版本 + 大小限制照搬 {@link ServerDataEnvelope}，与 Rust 侧 {@code MAX_PAYLOAD_BYTES=1024}
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
    public static final int MAX_PAYLOAD_BYTES = 1024;
    public static final int VFX_ANIM_PRIORITY_MIN = 100;
    public static final int VFX_ANIM_PRIORITY_MAX = 3999;
    public static final int VFX_FADE_TICKS_MAX = 40;

    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");
    private static final Pattern UUID_PATTERN =
        Pattern.compile("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$");
    private static final Pattern ANIM_ID_PATTERN = Pattern.compile("^[a-z0-9_]+:[a-z0-9_]+$");

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
                case "stop_anim" -> parseStopAnim(root);
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
