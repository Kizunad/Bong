package com.bong.client.ui.contract.surface;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** 稳定的语义动作标识及其窄的类型化参数模式。 */
public record UiActionSpec(
    String actionId,
    Map<String, ArgumentType> argsSchema,
    boolean available,
    String rejectionReason
) {
    public UiActionSpec {
        actionId = requireId(actionId, "actionId");
        argsSchema = copySchema(argsSchema);
        rejectionReason = normalize(rejectionReason);
        if (!available && rejectionReason == null) {
            throw new IllegalArgumentException("unavailable actions require a rejection reason");
        }
        if (available && rejectionReason != null) {
            throw new IllegalArgumentException("available actions cannot carry a rejection reason");
        }
    }

    public Validation validate(Map<String, ?> args) {
        if (!available) {
            return Validation.invalid(rejectionReason);
        }
        if (args == null) {
            return Validation.invalid("arguments must not be null");
        }
        for (String key : args.keySet()) {
            if (!argsSchema.containsKey(key)) {
                return Validation.invalid("unknown argument: " + key);
            }
        }
        for (String key : argsSchema.keySet()) {
            if (!args.containsKey(key)) {
                return Validation.invalid("missing argument: " + key);
            }
            if (!matches(argsSchema.get(key), args.get(key))) {
                return Validation.invalid("invalid argument type: " + key);
            }
        }
        return Validation.ok();
    }

    private static boolean matches(ArgumentType type, Object value) {
        if (value == null) {
            return false;
        }
        return switch (type) {
            case STRING -> value instanceof String;
            case INTEGER -> value instanceof Byte || value instanceof Short
                || value instanceof Integer || value instanceof Long;
            case BOOLEAN -> value instanceof Boolean;
        };
    }

    private static Map<String, ArgumentType> copySchema(Map<String, ArgumentType> schema) {
        Objects.requireNonNull(schema, "argsSchema must not be null");
        Map<String, ArgumentType> copy = new LinkedHashMap<>();
        schema.forEach((key, type) -> {
            copy.put(requireId(key, "argument name"), Objects.requireNonNull(type, "argument type must not be null"));
        });
        return Map.copyOf(copy);
    }

    private static String requireId(String value, String name) {
        Objects.requireNonNull(value, name + " must not be null");
        if (value.isBlank()) {
            throw new IllegalArgumentException(name + " must not be blank");
        }
        return value;
    }

    private static String normalize(String value) {
        return value == null || value.isBlank() ? null : value;
    }

    public enum ArgumentType {
        STRING,
        INTEGER,
        BOOLEAN
    }

    public record Validation(boolean valid, String reason) {
        public Validation {
            if (valid && reason != null) {
                throw new IllegalArgumentException("valid argument validation cannot carry a reason");
            }
            if (!valid && (reason == null || reason.isBlank())) {
                throw new IllegalArgumentException("invalid argument validation requires a reason");
            }
        }

        public static Validation ok() {
            return new Validation(true, null);
        }

        public static Validation invalid(String reason) {
            return new Validation(false, reason);
        }
    }
}
