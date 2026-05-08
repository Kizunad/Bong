package com.bong.client.combat.inspect;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Positioning;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/** Floating editor for a single technique's SkillConfig fields. */
public final class SkillConfigFloatingWindow {
    private final SkillConfigSchemaRegistry.SkillConfigSchema schema;
    private final FlowLayout root;
    private final JsonObject currentConfig;
    private final Consumer<JsonObject> onSave;
    private final Runnable onClose;
    private int x;
    private int y;

    public SkillConfigFloatingWindow(
        SkillConfigSchemaRegistry.SkillConfigSchema schema,
        JsonObject existingConfig,
        Consumer<JsonObject> onSave,
        Runnable onClose
    ) {
        this.schema = schema;
        this.currentConfig = existingConfig == null
            ? SkillConfigSchemaRegistry.defaultConfig(schema.skillId())
            : existingConfig.deepCopy();
        this.onSave = onSave;
        this.onClose = onClose;

        root = Containers.verticalFlow(Sizing.fixed(206), Sizing.content());
        root.surface(Surface.flat(0xFF111820).and(Surface.outline(0xFF54708A)));
        root.padding(Insets.of(5));
        root.gap(4);

        root.child(header());
        for (SkillConfigSchemaRegistry.ConfigField field : schema.fields()) {
            root.child(fieldRow(field));
        }
        root.child(actionRow());
    }

    public FlowLayout component() {
        return root;
    }

    public JsonObject currentConfig() {
        return currentConfig.deepCopy();
    }

    public void positionAt(int anchorX, int anchorY, int screenWidth, int screenHeight) {
        x = clamp(anchorX, 0, Math.max(0, screenWidth - 210));
        y = clamp(anchorY, 0, Math.max(0, screenHeight - 130));
        root.positioning(Positioning.absolute(x, y));
    }

    public void dragBy(int deltaX, int deltaY, int screenWidth, int screenHeight) {
        positionAt(x + deltaX, y + deltaY, screenWidth, screenHeight);
    }

    public static List<RenderField> renderFields(
        SkillConfigSchemaRegistry.SkillConfigSchema schema,
        JsonObject config
    ) {
        if (schema == null) return List.of();
        JsonObject effective = config == null ? SkillConfigSchemaRegistry.defaultConfig(schema.skillId()) : config;
        List<RenderField> fields = new ArrayList<>();
        for (SkillConfigSchemaRegistry.ConfigField field : schema.fields()) {
            fields.add(new RenderField(
                field.key(),
                field.label(),
                switch (field.kind()) {
                    case ENUM -> ControlKind.ENUM;
                    case MERIDIAN_ID -> ControlKind.MERIDIAN_ID;
                    case BOOL -> ControlKind.BOOL;
                },
                readCurrentValue(field, effective),
                field.options()
            ));
        }
        return List.copyOf(fields);
    }

    private FlowLayout header() {
        FlowLayout header = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(16));
        header.gap(4);
        LabelComponent title = Components.label(Text.literal("功法配置"));
        title.color(Color.ofArgb(0xFFE8D080));
        title.horizontalSizing(Sizing.fixed(160));
        LabelComponent close = Components.label(Text.literal("X"));
        close.color(Color.ofArgb(0xFFFF9090));
        close.cursorStyle(CursorStyle.HAND);
        close.mouseDown().subscribe((mx, my, button) -> {
            if (button != 0) return false;
            if (onClose != null) onClose.run();
            return true;
        });
        header.child(title);
        header.child(close);
        return header;
    }

    private FlowLayout fieldRow(SkillConfigSchemaRegistry.ConfigField field) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(20));
        row.gap(4);
        LabelComponent label = Components.label(Text.literal(field.label()));
        label.horizontalSizing(Sizing.fixed(62));
        label.color(Color.ofArgb(0xFFB8D0E8));
        LabelComponent value = Components.label(Text.literal(displayValue(field, readCurrentValue(field, currentConfig))));
        value.horizontalSizing(Sizing.fixed(126));
        value.color(Color.ofArgb(0xFFE0E0E0));
        value.cursorStyle(CursorStyle.HAND);
        value.mouseDown().subscribe((mx, my, button) -> {
            if (button != 0) return false;
            cycleField(field);
            value.text(Text.literal(displayValue(field, readCurrentValue(field, currentConfig))));
            return true;
        });
        row.child(label);
        row.child(value);
        return row;
    }

    private FlowLayout actionRow() {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(18));
        row.gap(6);
        LabelComponent save = actionLabel("保存", 0xFF80D890);
        save.mouseDown().subscribe((mx, my, button) -> {
            if (button != 0) return false;
            if (onSave != null) onSave.accept(currentConfig.deepCopy());
            if (onClose != null) onClose.run();
            return true;
        });
        LabelComponent cancel = actionLabel("取消", 0xFFB8B8B8);
        cancel.mouseDown().subscribe((mx, my, button) -> {
            if (button != 0) return false;
            if (onClose != null) onClose.run();
            return true;
        });
        row.child(save);
        row.child(cancel);
        return row;
    }

    private static LabelComponent actionLabel(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        label.horizontalSizing(Sizing.fixed(42));
        label.cursorStyle(CursorStyle.HAND);
        return label;
    }

    private void cycleField(SkillConfigSchemaRegistry.ConfigField field) {
        if (field.kind() == SkillConfigSchemaRegistry.FieldKind.BOOL) {
            boolean current = currentConfig.has(field.key())
                && currentConfig.get(field.key()).isJsonPrimitive()
                && currentConfig.get(field.key()).getAsJsonPrimitive().isBoolean()
                && currentConfig.get(field.key()).getAsBoolean();
            currentConfig.addProperty(field.key(), !current);
            return;
        }
        if (field.options().isEmpty()) return;
        String current = readCurrentValue(field, currentConfig);
        int index = -1;
        for (int i = 0; i < field.options().size(); i++) {
            if (field.options().get(i).value().equals(current)) {
                index = i;
                break;
            }
        }
        String next = field.options().get((index + 1) % field.options().size()).value();
        currentConfig.addProperty(field.key(), next);
    }

    private static String readCurrentValue(
        SkillConfigSchemaRegistry.ConfigField field,
        JsonObject config
    ) {
        JsonElement element = config == null ? null : config.get(field.key());
        if (element != null && element.isJsonPrimitive()) {
            JsonPrimitive primitive = element.getAsJsonPrimitive();
            if (field.kind() == SkillConfigSchemaRegistry.FieldKind.BOOL && primitive.isBoolean()) {
                return Boolean.toString(primitive.getAsBoolean());
            }
            if (primitive.isString() || primitive.isNumber() || primitive.isBoolean()) {
                return primitive.getAsString();
            }
        }
        if (field.defaultValue() != null && !field.defaultValue().isBlank()) {
            return field.defaultValue();
        }
        return field.options().isEmpty() ? "" : field.options().get(0).value();
    }

    private static String displayValue(SkillConfigSchemaRegistry.ConfigField field, String value) {
        if (field.kind() == SkillConfigSchemaRegistry.FieldKind.BOOL) {
            return Boolean.parseBoolean(value) ? "是" : "否";
        }
        return field.options().stream()
            .filter(option -> option.value().equals(value))
            .findFirst()
            .map(SkillConfigSchemaRegistry.Option::label)
            .orElse(value == null || value.isBlank() ? "-" : value);
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    public enum ControlKind {
        ENUM,
        MERIDIAN_ID,
        BOOL
    }

    public record RenderField(
        String key,
        String label,
        ControlKind controlKind,
        String currentValue,
        List<SkillConfigSchemaRegistry.Option> options
    ) {
        public RenderField {
            key = key == null ? "" : key;
            label = label == null ? "" : label;
            controlKind = controlKind == null ? ControlKind.ENUM : controlKind;
            currentValue = currentValue == null ? "" : currentValue;
            options = options == null ? List.of() : List.copyOf(options);
        }
    }
}
