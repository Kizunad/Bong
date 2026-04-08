package com.bong.client.state;

import java.util.Objects;
import java.util.Optional;
import java.util.Locale;

public final class NarrationState {
    private static final int MAX_TEXT_LENGTH = 500;

    private final Scope scope;
    private final String target;
    private final String text;
    private final Style style;
    private final int toastDurationMillis;

    private NarrationState(Scope scope, String target, String text, Style style, int toastDurationMillis) {
        this.scope = Objects.requireNonNull(scope, "scope");
        this.target = target;
        this.text = Objects.requireNonNull(text, "text");
        this.style = Objects.requireNonNull(style, "style");
        this.toastDurationMillis = Math.max(0, toastDurationMillis);
    }

    public static NarrationState empty() {
        return new NarrationState(Scope.BROADCAST, null, "", Style.NARRATION, 0);
    }

    public static NarrationState create(String scopeName, String target, String text, String styleName) {
        String normalizedText = normalizeText(text);
        if (normalizedText.isEmpty()) {
            return empty();
        }

        Scope scope = Scope.fromWireName(scopeName);
        Style style = Style.fromWireName(styleName);
        String normalizedTarget = scope == Scope.BROADCAST ? null : normalizeOptionalText(target);
        return new NarrationState(scope, normalizedTarget, normalizedText, style, style.toastDurationMillis());
    }

    private static String normalizeText(String value) {
        if (value == null) {
            return "";
        }

        String normalized = value.trim();
        if (normalized.length() <= MAX_TEXT_LENGTH) {
            return normalized;
        }
        return normalized.substring(0, MAX_TEXT_LENGTH);
    }

    private static String normalizeOptionalText(String value) {
        String normalized = normalizeText(value);
        return normalized.isEmpty() ? null : normalized;
    }

    public Scope scope() {
        return scope;
    }

    public Optional<String> target() {
        return Optional.ofNullable(target);
    }

    public String text() {
        return text;
    }

    public Style style() {
        return style;
    }

    public int toastDurationMillis() {
        return toastDurationMillis;
    }

    public boolean isEmpty() {
        return text.isEmpty();
    }

    public boolean isToastEligible() {
        return toastDurationMillis > 0;
    }

    public enum Scope {
        BROADCAST("broadcast"),
        ZONE("zone"),
        PLAYER("player");

        private final String wireName;

        Scope(String wireName) {
            this.wireName = wireName;
        }

        public static Scope fromWireName(String wireName) {
            for (Scope scope : values()) {
                if (scope.wireName.equals(normalizeWireName(wireName))) {
                    return scope;
                }
            }
            return BROADCAST;
        }

        public String wireName() {
            return wireName;
        }
    }

    public enum Style {
        SYSTEM_WARNING("system_warning", 5_000),
        PERCEPTION("perception", 0),
        NARRATION("narration", 0),
        ERA_DECREE("era_decree", 8_000);

        private final String wireName;
        private final int toastDurationMillis;

        Style(String wireName, int toastDurationMillis) {
            this.wireName = wireName;
            this.toastDurationMillis = toastDurationMillis;
        }

        public static Style fromWireName(String wireName) {
            for (Style style : values()) {
                if (style.wireName.equals(normalizeWireName(wireName))) {
                    return style;
                }
            }
            return NARRATION;
        }

        public String wireName() {
            return wireName;
        }

        public int toastDurationMillis() {
            return toastDurationMillis;
        }
    }

    private static String normalizeWireName(String wireName) {
        return wireName == null ? "" : wireName.trim().toLowerCase(Locale.ROOT);
    }
}
