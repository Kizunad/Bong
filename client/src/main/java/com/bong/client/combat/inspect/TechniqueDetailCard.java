package com.bong.client.combat.inspect;

import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillBarStore;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.text.Text;

import java.util.function.Consumer;

/** Compact detail card for the selected combat technique. */
public final class TechniqueDetailCard {
    private final FlowLayout root;
    private final LabelComponent title;
    private final LabelComponent description;
    private final LabelComponent requirements;
    private final LabelComponent numbers;
    private final LabelComponent binding;
    private final LabelComponent configGear;
    private TechniquesListPanel.Technique currentTechnique;

    public TechniqueDetailCard() {
        this(null);
    }

    public TechniqueDetailCard(Consumer<TechniquesListPanel.Technique> onConfigure) {
        root = Containers.verticalFlow(Sizing.fixed(176), Sizing.content());
        root.surface(Surface.flat(0xFF14181E).and(Surface.outline(0xFF3A4658)));
        root.padding(Insets.of(4));
        root.gap(3);

        title = Components.label(Text.literal("功法详情"));
        title.color(Color.ofArgb(0xFFE8D080));
        description = Components.label(Text.literal("选择左侧功法查看描述、需求与招式数值。"));
        description.color(Color.ofArgb(0xFFB8B8B8));
        requirements = Components.label(Text.literal(""));
        requirements.color(Color.ofArgb(0xFF9BC0E8));
        numbers = Components.label(Text.literal(""));
        numbers.color(Color.ofArgb(0xFFC8C8C8));
        binding = Components.label(Text.literal(""));
        binding.color(Color.ofArgb(0xFFE0B060));
        configGear = Components.label(Text.literal(""));
        configGear.color(Color.ofArgb(0xFFE0B060));
        configGear.cursorStyle(CursorStyle.HAND);
        configGear.mouseDown().subscribe((mx, my, button) -> {
            if (button != 0 || currentTechnique == null) return false;
            if (!SkillConfigSchemaRegistry.hasSchema(currentTechnique.id())) return false;
            if (CastStateStore.snapshot().isCasting()) return false;
            if (onConfigure != null) onConfigure.accept(currentTechnique);
            return true;
        });

        root.child(title);
        root.child(description);
        root.child(requirements);
        root.child(numbers);
        root.child(binding);
        root.child(configGear);
    }

    public FlowLayout component() {
        return root;
    }

    public void refresh(TechniquesListPanel.Technique technique) {
        currentTechnique = technique;
        if (technique == null) {
            title.text(Text.literal("功法详情"));
            description.text(Text.literal("选择左侧功法查看描述、需求与招式数值。"));
            requirements.text(Text.literal(""));
            numbers.text(Text.literal(""));
            binding.text(Text.literal(""));
            configGear.text(Text.literal(""));
            return;
        }
        title.text(Text.literal(technique.grade().label() + " · " + technique.displayName()));
        title.color(Color.ofArgb(technique.grade().color()));
        description.text(Text.literal(shorten(technique.description(), 52)));
        requirements.text(Text.literal("需求: " + realmText(technique) + " / 经脉 " + requiredMeridianText(technique)));
        numbers.text(Text.literal(technique.proficiencyLabel()
            + " " + Math.round(technique.proficiency() * 100.0f) + "%"
            + " · 真元 " + String.format(java.util.Locale.ROOT, "%.2f", technique.qiCost())
            + " · cast " + technique.castTicks() + "t"
            + " · cd " + technique.cooldownTicks() + "t"
            + " · 距离 " + String.format(java.util.Locale.ROOT, "%.1f", technique.range())));
        int boundSlot = SkillBarStore.findSkill(technique.id());
        binding.text(Text.literal(boundSlot >= 0 ? "已绑定: 左侧槽 " + (boundSlot + 1) : "未绑定: 拖到左侧 1-9 槽"));
        refreshConfigGear(technique);
    }

    private void refreshConfigGear(TechniquesListPanel.Technique technique) {
        if (!SkillConfigSchemaRegistry.hasSchema(technique.id())) {
            configGear.text(Text.literal(""));
            configGear.tooltip(Text.literal(""));
            return;
        }
        configGear.text(Text.literal("⚙ 配置"));
        if (CastStateStore.snapshot().isCasting()) {
            configGear.color(Color.ofArgb(0xFF777777));
            configGear.tooltip(Text.literal("施法中不可改配置"));
        } else {
            configGear.color(Color.ofArgb(0xFFE0B060));
            configGear.tooltip(Text.literal("配置功法参数"));
        }
    }

    private static String realmText(TechniquesListPanel.Technique technique) {
        return technique.requiredRealm().isBlank() ? "无" : technique.requiredRealm();
    }

    private static String requiredMeridianText(TechniquesListPanel.Technique technique) {
        if (technique.requiredMeridians().isEmpty()) return "无";
        return technique.requiredMeridians().stream()
            .map(TechniquesListPanel.RequiredMeridian::channel)
            .filter(channel -> !channel.isBlank())
            .limit(4)
            .reduce((a, b) -> a + ", " + b)
            .orElse("无");
    }

    private static String shorten(String text, int max) {
        if (text == null || text.isBlank()) return "无描述。";
        String trimmed = text.trim();
        return trimmed.length() <= max ? trimmed : trimmed.substring(0, Math.max(0, max - 1)) + "…";
    }
}
