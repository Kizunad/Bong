package com.bong.client.network;

import com.bong.client.state.RealmCollapseHudState;
import com.bong.client.state.VisualEffectState;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Objects;
import java.util.Locale;
import java.util.function.LongSupplier;

public final class EventAlertHandler implements ServerDataHandler {
    static final int INFO_COLOR = 0x9FD3FF;
    static final int WARNING_COLOR = 0xFFAA55;
    static final int CRITICAL_COLOR = 0xFF5555;

    private final LongSupplier nowMillisSupplier;

    public EventAlertHandler() {
        this(System::currentTimeMillis);
    }

    EventAlertHandler(LongSupplier nowMillisSupplier) {
        this.nowMillisSupplier = Objects.requireNonNull(nowMillisSupplier, "nowMillisSupplier");
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String title = firstNonBlank(readOptionalString(payload, "title"), deriveTitleFromEvent(readOptionalString(payload, "event")));
        String message = normalizeText(readOptionalString(payload, "message"));
        if (title == null || title.isEmpty() || message.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring event_alert payload because required fields 'message' and either 'title' or 'event' are missing or invalid"
            );
        }

        Severity severity = Severity.fromWireName(readOptionalString(payload, "severity"));
        long durationMillis = normalizeDuration(readOptionalLong(payload, "duration_ms"), severity.defaultDurationMillis());
        ServerDataDispatch.ToastSpec toastSpec = new ServerDataDispatch.ToastSpec(
            formatToastText(title, message),
            severity.color(),
            durationMillis
        );
        VisualEffectState visualEffectState = parseEffectHint(payload.get("effect"), severity, durationMillis, nowMillisSupplier.getAsLong());
        RealmCollapseHudState realmCollapseHudState = parseRealmCollapseHudState(
            readOptionalString(payload, "event"),
            message,
            readOptionalString(payload, "zone"),
            readOptionalLong(payload, "duration_ticks"),
            nowMillisSupplier.getAsLong()
        );

        String logMessage = "Routed event_alert payload with severity '"
            + severity.wireName()
            + "'"
            + (visualEffectState.isEmpty() ? "" : " and effect hint");
        return ServerDataDispatch.handledWithEventAlert(envelope.type(), toastSpec, visualEffectState, realmCollapseHudState, logMessage);
    }

    private static RealmCollapseHudState parseRealmCollapseHudState(
        String eventName,
        String message,
        String zone,
        Long durationTicks,
        long nowMillis
    ) {
        if (!"realm_collapse".equals(normalizeText(eventName).toLowerCase(Locale.ROOT))) {
            return RealmCollapseHudState.empty();
        }
        if (durationTicks == null || durationTicks <= 0L || durationTicks > Integer.MAX_VALUE) {
            return RealmCollapseHudState.empty();
        }
        return RealmCollapseHudState.create(zone, message, nowMillis, durationTicks.intValue());
    }

    static String formatToastText(String title, String message) {
        String normalizedTitle = normalizeText(title);
        String normalizedMessage = normalizeText(message);
        if (normalizedTitle.isEmpty()) {
            return normalizedMessage;
        }
        if (normalizedMessage.isEmpty()) {
            return normalizedTitle;
        }
        return normalizedTitle + "：" + normalizedMessage;
    }

    private static VisualEffectState parseEffectHint(
        JsonElement effectElement,
        Severity severity,
        long fallbackDurationMillis,
        long nowMillis
    ) {
        if (effectElement == null || effectElement.isJsonNull()) {
            return VisualEffectState.none();
        }

        String effectType = null;
        Double intensity = null;
        Long durationMillis = null;

        if (effectElement.isJsonPrimitive()) {
            JsonPrimitive primitive = effectElement.getAsJsonPrimitive();
            if (primitive.isString()) {
                effectType = primitive.getAsString();
            }
        } else if (effectElement.isJsonObject()) {
            JsonObject effectObject = effectElement.getAsJsonObject();
            effectType = firstNonBlank(
                readOptionalString(effectObject, "type"),
                readOptionalString(effectObject, "hint"),
                readOptionalString(effectObject, "name")
            );
            intensity = readOptionalDouble(effectObject, "intensity");
            durationMillis = firstPositive(readOptionalLong(effectObject, "duration_ms"), readOptionalLong(effectObject, "duration"));
        }

        return VisualEffectState.create(
            effectType,
            intensity == null ? severity.defaultIntensity() : intensity,
            durationMillis == null ? fallbackDurationMillis : durationMillis,
            nowMillis
        );
    }

    private static long normalizeDuration(Long candidateDurationMillis, long fallbackDurationMillis) {
        if (candidateDurationMillis == null || candidateDurationMillis <= 0L) {
            return fallbackDurationMillis;
        }
        return candidateDurationMillis;
    }

    private static String deriveTitleFromEvent(String eventName) {
        String normalizedEventName = normalizeText(eventName);
        if (normalizedEventName.isEmpty()) {
            return null;
        }

        StringBuilder titleBuilder = new StringBuilder();
        for (String rawSegment : normalizedEventName.split("_")) {
            String normalizedSegment = normalizeText(rawSegment);
            if (normalizedSegment.isEmpty()) {
                continue;
            }

            String lowerCaseSegment = normalizedSegment.toLowerCase(Locale.ROOT);
            if (titleBuilder.length() > 0) {
                titleBuilder.append(' ');
            }
            titleBuilder.append(Character.toUpperCase(lowerCaseSegment.charAt(0)));
            if (lowerCaseSegment.length() > 1) {
                titleBuilder.append(lowerCaseSegment.substring(1));
            }
        }

        String derivedTitle = titleBuilder.toString();
        return derivedTitle.isEmpty() ? null : derivedTitle;
    }

    private static String firstNonBlank(String... candidates) {
        if (candidates == null) {
            return null;
        }
        for (String candidate : candidates) {
            String normalized = normalizeText(candidate);
            if (!normalized.isEmpty()) {
                return normalized;
            }
        }
        return null;
    }

    private static Long firstPositive(Long... candidates) {
        if (candidates == null) {
            return null;
        }
        for (Long candidate : candidates) {
            if (candidate != null && candidate > 0L) {
                return candidate;
            }
        }
        return null;
    }

    private static String normalizeText(String value) {
        return value == null ? "" : value.trim();
    }

    private static String readOptionalString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static Double readOptionalDouble(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        return primitive.getAsDouble();
    }

    private static Long readOptionalLong(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        return primitive.getAsLong();
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }

    enum Severity {
        INFO("info", INFO_COLOR, 3_500L, 0.35),
        WARNING("warning", WARNING_COLOR, 5_000L, 0.6),
        CRITICAL("critical", CRITICAL_COLOR, 6_500L, 0.9);

        private final String wireName;
        private final int color;
        private final long defaultDurationMillis;
        private final double defaultIntensity;

        Severity(String wireName, int color, long defaultDurationMillis, double defaultIntensity) {
            this.wireName = wireName;
            this.color = color;
            this.defaultDurationMillis = defaultDurationMillis;
            this.defaultIntensity = defaultIntensity;
        }

        static Severity fromWireName(String wireName) {
            String normalizedWireName = wireName == null ? "" : wireName.trim().toLowerCase(Locale.ROOT);
            for (Severity severity : values()) {
                if (severity.wireName.equals(normalizedWireName)) {
                    return severity;
                }
            }
            return WARNING;
        }

        String wireName() {
            return wireName;
        }

        int color() {
            return color;
        }

        long defaultDurationMillis() {
            return defaultDurationMillis;
        }

        double defaultIntensity() {
            return defaultIntensity;
        }
    }
}
