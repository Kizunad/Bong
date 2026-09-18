package com.bong.client.combat.inspect;

import com.bong.client.ui.intent.UiIntentResult;
import com.google.gson.JsonObject;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.text.Text;
import java.util.function.Function;

/** 每窗独立草稿，服务端快照只能初始化，不能在输入期间覆盖玩家选择。 */
public final class SkillConfigContent {
    private final FlowLayout root = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
    private final JsonObject draft;
    private final LabelComponent status = Components.label(Text.empty());

    public SkillConfigContent(SkillConfigSchemaRegistry.SkillConfigSchema schema, JsonObject config,
                              Function<JsonObject, UiIntentResult> save) {
        draft = config.deepCopy();
        root.gap(6);
        for (var field : schema.fields()) {
            root.child(Components.label(Text.literal(field.label())).color(Color.ofRgb(0xC2CBBE)));
            if (field.kind() == SkillConfigSchemaRegistry.FieldKind.BOOL) {
                var check = Components.checkbox(Text.literal(field.label()));
                check.checked(Boolean.parseBoolean(value(field)));
                check.onChanged(enabled -> draft.addProperty(field.key(), enabled));
                root.child(check);
            } else {
                var choices = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
                choices.gap(2);
                var select = button(label(field, value(field)), () -> {});
                select.id("config-" + field.key());
                var options = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
                options.gap(2);
                var scroll = Containers.verticalScroll(Sizing.fill(100),
                    Sizing.fixed(Math.min(112, field.options().size() * 24)), options);
                scroll.scrollbarThiccness(4);
                select.onPress(ignored -> {
                    if (scroll.parent() == choices) choices.removeChild(scroll);
                    else choices.child(scroll);
                });
                for (var option : field.options()) {
                    var choice = button(option.label(), () -> {
                        draft.addProperty(field.key(), option.value());
                        select.setMessage(Text.literal(option.label()));
                        choices.removeChild(scroll);
                    });
                    choice.id("option-" + option.value());
                    options.child(choice);
                }
                choices.child(select);
                root.child(choices);
            }
        }
        var submit = button("保存", () -> {
            var result = save.apply(draft.deepCopy());
            if (result.kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) {
                status.text(Text.literal(result.reason()));
            }
        });
        submit.id("config-save");
        root.child(submit);
        status.horizontalSizing(Sizing.fill(100));
        status.color(Color.ofRgb(0xDF9991));
        root.child(status);
    }

    public FlowLayout component() { return root; }

    private String value(SkillConfigSchemaRegistry.ConfigField field) {
        var value = draft.get(field.key());
        return value != null && value.isJsonPrimitive() ? value.getAsString() : field.defaultValue();
    }

    private static String label(SkillConfigSchemaRegistry.ConfigField field, String value) {
        return field.options().stream().filter(option -> option.value().equals(value))
            .map(SkillConfigSchemaRegistry.Option::label).findFirst().orElse("未选择");
    }

    private static ButtonComponent button(String label, Runnable action) {
        var button = Components.button(Text.literal(label), ignored -> action.run());
        button.sizing(Sizing.fill(100), Sizing.fixed(22));
        button.textShadow(false);
        button.renderer(ButtonComponent.Renderer.flat(0xFF303A39, 0xFF485652, 0xFF242C2B));
        return button;
    }
}
