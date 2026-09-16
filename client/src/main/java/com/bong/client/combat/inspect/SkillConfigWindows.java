package com.bong.client.combat.inspect;

import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.window.UiWindowDefinition;
import com.bong.client.ui.window.UiWindowManager;
import com.google.gson.JsonObject;
import java.util.List;
import java.util.Set;
import java.util.function.BooleanSupplier;
import java.util.function.Supplier;

/** 配置窗口只拥有编辑生命周期；选择另一功法不会改变已打开窗口的目标。 */
public final class SkillConfigWindows {
    public static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "skill-config", "skill-config", 220, 150, Set.of(UiWindowDefinition.Capability.WINDOW));
    private final UiWindowManager manager;
    private final Supplier<List<TechniquesListPanel.Technique>> techniques;
    private final BooleanSupplier casting;
    private final UiIntentSink<TechniqueIntent> intents;

    public SkillConfigWindows(UiWindowManager manager, Supplier<List<TechniquesListPanel.Technique>> techniques,
                              BooleanSupplier casting, UiIntentSink<TechniqueIntent> intents) {
        this.manager = manager;
        this.techniques = techniques;
        this.casting = casting;
        this.intents = intents;
    }

    public UiWindowManager.WindowState open(String skillId, UiWindowManager.Rect bounds) {
        if (!available(skillId)) return null;
        return manager.openOrFocus(DEFINITION, manager.key(DEFINITION.windowType(), skillId), bounds);
    }

    public void refresh() {
        for (var window : manager.snapshot()) {
            if (window.definition().equals(DEFINITION) && !available(window.key().identity())) {
                manager.close(window.key());
            }
        }
    }

    public UiIntentResult save(UiWindowManager.WindowState owner, JsonObject draft) {
        // 检查实例 scope，而非仅检查 key；同一技能关闭后重开时旧回调仍必须失效。
        if (owner.closed() || !owner.definition().equals(DEFINITION)
            || !manager.contains(owner.key()) || !available(owner.key().identity())) {
            return UiIntentResult.rejected("配置已失效或正在施法");
        }
        var result = intents.dispatch(new TechniqueIntent.Configure(owner.key().identity(), draft));
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) manager.close(owner.key());
        return result;
    }

    private boolean available(String id) {
        return !casting.getAsBoolean() && SkillConfigSchemaRegistry.hasSchema(id)
            && techniques.get().stream().anyMatch(technique -> technique.id().equals(id));
    }
}
