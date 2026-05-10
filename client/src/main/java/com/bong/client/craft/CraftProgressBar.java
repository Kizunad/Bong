package com.bong.client.craft;

import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.text.Text;

/** 中栏制作进度条。 */
public final class CraftProgressBar {
    private final FlowLayout root;
    private final FlowLayout fill;
    private final LabelComponent label;

    public CraftProgressBar() {
        root = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        root.gap(2);
        label = Components.label(Text.literal("未开始"));
        label.color(Color.ofArgb(0xFFB8B8C8));
        root.child(label);

        FlowLayout track = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(9));
        track.surface(Surface.flat(0xFF101018).and(Surface.outline(0xFF404050)));
        fill = Containers.horizontalFlow(Sizing.fill(0), Sizing.fill(100));
        fill.surface(Surface.flat(0xFF44AA44));
        track.child(fill);
        root.child(track);
    }

    public FlowLayout root() {
        return root;
    }

    public void refresh(CraftRecipe selected, CraftSessionStateView state) {
        if (state == null || !state.active()) {
            label.text(Text.literal("未开始"));
            fill.horizontalSizing(Sizing.fill(0));
            return;
        }
        String activeId = state.recipeId().orElse("");
        boolean sameRecipe = selected != null && selected.id().equals(activeId);
        int pct = Math.round(state.progress() * 100);
        String prefix = sameRecipe ? "制作中" : "其他配方制作中";
        String batch = state.totalCount() > 1
            ? String.format(" · %d/%d", state.completedCount(), state.totalCount())
            : "";
        label.text(Text.literal(String.format("%s%s · %d%% · 剩 %ds", prefix, batch, pct, state.remainingSeconds())));
        fill.horizontalSizing(Sizing.fill(Math.max(0, Math.min(100, pct))));
    }
}
