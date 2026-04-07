package com.bong.client.network.handlers;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;

import java.util.ArrayList;
import java.util.List;

public final class NarrationPayloadParser {
    private static final int COLOR_WHITE = 0xFFFFFF;
    private static final int COLOR_GRAY = 0xAAAAAA;
    private static final int COLOR_WARNING = 0xFF5555;
    private static final int COLOR_ERA = 0xFFAA00;
    private static final long WARNING_TOAST_DURATION_MS = 5_000L;
    private static final long ERA_TOAST_DURATION_MS = 8_000L;

    private NarrationPayloadParser() {
    }

    public static ParseResult parse(String jsonPayload) {
        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return ParseResult.error("Narration payload root must be an object");
            }

            JsonObject rootObject = rootElement.getAsJsonObject();
            JsonElement narrationsElement = rootObject.get("narrations");
            if (narrationsElement == null || !narrationsElement.isJsonArray()) {
                return ParseResult.error("Narration payload missing array field 'narrations'");
            }

            JsonArray narrationsArray = narrationsElement.getAsJsonArray();
            List<RenderedNarration> renderedNarrations = new ArrayList<>();
            for (JsonElement narrationElement : narrationsArray) {
                RenderedNarration renderedNarration = parseNarration(narrationElement);
                if (renderedNarration != null) {
                    renderedNarrations.add(renderedNarration);
                }
            }

            return ParseResult.success(renderedNarrations);
        } catch (JsonParseException | IllegalStateException exception) {
            return ParseResult.error("Malformed narration payload");
        }
    }

    private static RenderedNarration parseNarration(JsonElement narrationElement) {
        if (!narrationElement.isJsonObject()) {
            return null;
        }

        JsonObject narrationObject = narrationElement.getAsJsonObject();
        String text = getString(narrationObject, "text");
        if (text == null) {
            return null;
        }

        String style = getString(narrationObject, "style");
        if (style == null) {
            return defaultNarration("unknown", text);
        }

        return switch (style) {
            case "narration" -> new RenderedNarration(
                style,
                text,
                "[叙事]",
                COLOR_WHITE,
                COLOR_WHITE,
                false,
                null
            );
            case "perception" -> new RenderedNarration(
                style,
                text,
                "[感知]",
                COLOR_GRAY,
                COLOR_GRAY,
                false,
                null
            );
            case "system_warning" -> new RenderedNarration(
                style,
                text,
                "[天道警示]",
                COLOR_WARNING,
                COLOR_WARNING,
                true,
                new ToastSpec(text, COLOR_WARNING, WARNING_TOAST_DURATION_MS)
            );
            case "era_decree" -> new RenderedNarration(
                style,
                text,
                "[时代]",
                COLOR_ERA,
                COLOR_ERA,
                true,
                new ToastSpec(text, COLOR_ERA, ERA_TOAST_DURATION_MS)
            );
            default -> defaultNarration(style, text);
        };
    }

    private static RenderedNarration defaultNarration(String style, String text) {
        return new RenderedNarration(style, text, null, COLOR_WHITE, COLOR_WHITE, false, null);
    }

    private static String getString(JsonObject object, String key) {
        JsonElement value = object.get(key);
        if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) {
            return null;
        }

        return value.getAsString();
    }

    public record ParseResult(boolean success, List<RenderedNarration> narrations, String errorMessage) {
        private static ParseResult success(List<RenderedNarration> narrations) {
            return new ParseResult(true, List.copyOf(narrations), null);
        }

        private static ParseResult error(String errorMessage) {
            return new ParseResult(false, List.of(), errorMessage);
        }
    }

    public record RenderedNarration(
        String style,
        String text,
        String chatLabel,
        int labelColor,
        int textColor,
        boolean boldLabel,
        ToastSpec toast
    ) {
        public String plainChatText() {
            if (chatLabel == null || chatLabel.isBlank()) {
                return text;
            }

            return chatLabel + " " + text;
        }
    }

    public record ToastSpec(String text, int color, long durationMs) {
    }
}
