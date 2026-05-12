package com.bong.client.combat.inspect;

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
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.gui.tooltip.TooltipComponent;
import net.minecraft.text.Text;

import java.util.function.Consumer;

/** Single row in the techniques list. */
public final class TechniqueRowComponent {
    private final TechniquesListPanel.Technique technique;
    private final FlowLayout root;
    private final LabelComponent grade;
    private final LabelComponent name;
    private final LabelComponent proficiency;
    private final LabelComponent slot;
    private final LabelComponent lock;

    public TechniqueRowComponent(
        TechniquesListPanel.Technique technique,
        Consumer<TechniquesListPanel.Technique> onSelect,
        Consumer<TechniquesListPanel.Technique> onHover,
        Runnable onLeave
    ) {
        this.technique = technique;
        root = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(20));
        root.surface(Surface.flat(0xFF181818));
        root.padding(Insets.of(2, 2, 4, 4));
        root.gap(4);
        root.verticalAlignment(VerticalAlignment.CENTER);
        root.cursorStyle(CursorStyle.HAND);

        grade = Components.label(Text.literal(technique.grade().label()));
        grade.horizontalSizing(Sizing.fixed(34));
        root.child(grade);
        name = Components.label(Text.literal(technique.displayName()));
        name.horizontalSizing(Sizing.fixed(66));
        root.child(name);
        proficiency = Components.label(Text.literal(""));
        proficiency.horizontalSizing(Sizing.fixed(34));
        root.child(proficiency);
        slot = Components.label(Text.literal(""));
        slot.horizontalSizing(Sizing.fixed(20));
        root.child(slot);
        lock = Components.label(Text.literal(""));
        root.child(lock);

        root.mouseDown().subscribe((mx, my, btn) -> {
            if (btn != 0) return false;
            if (onSelect != null) onSelect.accept(technique);
            return true;
        });
        root.mouseEnter().subscribe(() -> {
            if (onHover != null) onHover.accept(technique);
        });
        root.mouseLeave().subscribe(() -> {
            if (onLeave != null) onLeave.run();
        });
        refresh(false, "");
    }

    public FlowLayout component() {
        return root;
    }

    public TechniquesListPanel.Technique technique() {
        return technique;
    }

    public void refresh(boolean selected, String lockReason) {
        boolean locked = lockReason != null && !lockReason.isBlank();
        root.surface(selected
            ? Surface.flat(0xFF20221A).and(Surface.outline(0xFFE0B060))
            : Surface.flat(locked ? 0xFF141414 : 0xFF181818));
        grade.color(Color.ofArgb(locked ? 0xFF777777 : technique.grade().color()));
        name.color(Color.ofArgb(locked ? 0xFF777777 : 0xFFE0E0E0));
        proficiency.text(Text.literal(technique.proficiencyLabel()));
        proficiency.tooltip(Text.literal(Math.round(technique.proficiency() * 100.0f) + "%"));
        proficiency.color(Color.ofArgb(locked ? 0xFF666666 : 0xFF88CCAA));
        int boundSlot = SkillBarStore.findSkill(technique.id());
        slot.text(Text.literal(boundSlot >= 0 ? String.valueOf(boundSlot + 1) : "-"));
        slot.color(Color.ofArgb(boundSlot >= 0 ? 0xFFE0B060 : 0xFF666666));
        lock.text(Text.literal(locked ? "锁" : ""));
        lock.color(Color.ofArgb(0xFFCC6666));
        if (locked) {
            lock.tooltip(Text.literal(lockReason));
        } else {
            lock.tooltip((java.util.List<TooltipComponent>) null);
        }
    }
}
