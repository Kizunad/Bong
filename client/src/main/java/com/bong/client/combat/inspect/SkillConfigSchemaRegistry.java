package com.bong.client.combat.inspect;

import com.bong.client.network.ClientRequestProtocol;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/** Static client mirror for SkillConfig schemas that affect inspect UI rendering. */
public final class SkillConfigSchemaRegistry {
    private static final Map<String, SkillConfigSchema> SCHEMAS = Map.of(
        "zhenmai.sever_chain",
        new SkillConfigSchema(
            "zhenmai.sever_chain",
            List.of(
                ConfigField.meridian(
                    "meridian_id",
                    "选定经脉",
                    allMeridianOptions(),
                    true,
                    "Lung"
                ),
                ConfigField.enumeration(
                    "backfire_kind",
                    "反震类型",
                    List.of(
                        new Option("real_yuan", "真元 real_yuan"),
                        new Option("physical_carrier", "载体 physical_carrier"),
                        new Option("tainted_yuan", "污染 tainted_yuan"),
                        new Option("array", "阵法 array")
                    ),
                    true,
                    "real_yuan"
                )
            )
        )
    );

    private SkillConfigSchemaRegistry() {
    }

    public static boolean hasSchema(String skillId) {
        return schemaFor(skillId).isPresent();
    }

    public static Optional<SkillConfigSchema> schemaFor(String skillId) {
        if (skillId == null || skillId.isBlank()) return Optional.empty();
        return Optional.ofNullable(SCHEMAS.get(skillId));
    }

    public static JsonObject defaultConfig(String skillId) {
        JsonObject config = new JsonObject();
        schemaFor(skillId).ifPresent(schema -> {
            for (ConfigField field : schema.fields()) {
                if (field.defaultValue() != null && !field.defaultValue().isBlank()) {
                    if (field.kind() == FieldKind.BOOL) {
                        config.addProperty(field.key(), Boolean.parseBoolean(field.defaultValue()));
                    } else {
                        config.addProperty(field.key(), field.defaultValue());
                    }
                }
            }
        });
        return config;
    }

    public static String missingRequiredReason(String skillId, JsonObject config) {
        Optional<SkillConfigSchema> schema = schemaFor(skillId);
        if (schema.isEmpty()) return "";
        JsonObject effective = config == null ? new JsonObject() : config;
        for (ConfigField field : schema.get().fields()) {
            if (!field.required()) continue;
            if (!effective.has(field.key()) || effective.get(field.key()).isJsonNull()) {
                return "配置未完成: " + field.label();
            }
        }
        return "";
    }

    public enum FieldKind {
        ENUM,
        MERIDIAN_ID,
        BOOL
    }

    public record SkillConfigSchema(String skillId, List<ConfigField> fields) {
        public SkillConfigSchema {
            skillId = skillId == null ? "" : skillId;
            fields = fields == null ? List.of() : List.copyOf(fields);
        }
    }

    public record ConfigField(
        String key,
        String label,
        FieldKind kind,
        List<Option> options,
        boolean required,
        String defaultValue
    ) {
        public ConfigField {
            key = key == null ? "" : key;
            label = label == null ? key : label;
            kind = kind == null ? FieldKind.ENUM : kind;
            options = options == null ? List.of() : List.copyOf(options);
            defaultValue = defaultValue == null ? "" : defaultValue;
        }

        static ConfigField enumeration(
            String key,
            String label,
            List<Option> options,
            boolean required,
            String defaultValue
        ) {
            return new ConfigField(key, label, FieldKind.ENUM, options, required, defaultValue);
        }

        static ConfigField meridian(
            String key,
            String label,
            List<Option> options,
            boolean required,
            String defaultValue
        ) {
            return new ConfigField(key, label, FieldKind.MERIDIAN_ID, options, required, defaultValue);
        }

        static ConfigField bool(String key, String label, boolean required, boolean defaultValue) {
            return new ConfigField(
                key,
                label,
                FieldKind.BOOL,
                List.of(new Option("false", "否"), new Option("true", "是")),
                required,
                Boolean.toString(defaultValue)
            );
        }
    }

    public record Option(String value, String label) {
        public Option {
            value = value == null ? "" : value;
            label = label == null || label.isBlank() ? value : label;
        }
    }

    private static List<Option> allMeridianOptions() {
        List<Option> out = new ArrayList<>();
        for (ClientRequestProtocol.MeridianId meridian : ClientRequestProtocol.MeridianId.values()) {
            out.add(new Option(meridian.name(), meridianLabel(meridian.name()) + " " + meridian.name()));
        }
        return List.copyOf(out);
    }

    private static String meridianLabel(String wire) {
        return switch (wire) {
            case "Lung" -> "肺经";
            case "LargeIntestine" -> "大肠经";
            case "Stomach" -> "胃经";
            case "Spleen" -> "脾经";
            case "Heart" -> "心经";
            case "SmallIntestine" -> "小肠经";
            case "Bladder" -> "膀胱经";
            case "Kidney" -> "肾经";
            case "Pericardium" -> "心包经";
            case "TripleEnergizer" -> "三焦经";
            case "Gallbladder" -> "胆经";
            case "Liver" -> "肝经";
            case "Ren" -> "任脉";
            case "Du" -> "督脉";
            case "Chong" -> "冲脉";
            case "Dai" -> "带脉";
            case "YinQiao" -> "阴跷脉";
            case "YangQiao" -> "阳跷脉";
            case "YinWei" -> "阴维脉";
            case "YangWei" -> "阳维脉";
            default -> wire;
        };
    }
}
