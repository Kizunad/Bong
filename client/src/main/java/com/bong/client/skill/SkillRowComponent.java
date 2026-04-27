package com.bong.client.skill;

import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

/**
 * plan-skill-v1 §5.1 左列每行：`{ 图标 placeholder · 中文名 · Lv.X / cap Y · XP 进度条 · 最近 +XP }`。
 * 三行固定（herbalism / alchemy / forging）。
 *
 * <p>图标贴图等视觉资源待 P4+ 接入；当前只用文本 placeholder，避免 MVP 阶段阻塞资产。
 *
 * <p>刷新逻辑走 {@link #update(SkillSetSnapshot.Entry, long)} —— 由 InspectScreen 订阅
 * {@link SkillSetStore} 后统一推数据。
 */
public final class SkillRowComponent {
    private static final int COLOR_NAME = 0xFFCCCCCC;
    private static final int COLOR_LV = 0xFFE0B060;
    private static final int COLOR_CAPPED = 0xFF705030;
    private static final int COLOR_BG = 0xFF181818;
    private static final int COLOR_BG_SELECTED = 0xFF1F2B1F;
    private static final int COLOR_BORDER_SELECTED = 0xFF5C7A54;
    private static final int COLOR_XP_TRACK = 0xFF202020;
    private static final int COLOR_XP_FILL = 0xFF558866;
    private static final int COLOR_GAIN = 0xFF70C88C;

    /** plan §5.1 "最近 +XP" 在 3s 内显示，超时淡出到空串。 */
    private static final long RECENT_GAIN_WINDOW_MS = 3_000L;

    private final SkillId id;
    private final FlowLayout root;
    private final LabelComponent nameLabel;
    private final LabelComponent lvLabel;
    private final FlowLayout xpTrack;
    private final FlowLayout xpFill;
    private final LabelComponent gainLabel;

    public SkillRowComponent(SkillId id) {
        this.id = id;
        root = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(18));
        root.surface(Surface.flat(COLOR_BG));
        root.padding(Insets.of(2, 2, 4, 4));
        root.gap(6);
        root.verticalAlignment(VerticalAlignment.CENTER);

        // 图标 placeholder —— P4+ 换贴图；此处一个 12×12 彩色方块先占位。
        FlowLayout icon = Containers.verticalFlow(Sizing.fixed(12), Sizing.fixed(12));
        icon.surface(Surface.flat(0xFF404040));
        root.child(icon);

        nameLabel = Components.label(Text.literal(id.displayName()));
        nameLabel.color(Color.ofArgb(COLOR_NAME));
        nameLabel.horizontalSizing(Sizing.fixed(36));
        root.child(nameLabel);

        lvLabel = Components.label(Text.literal("Lv.0 / cap 10"));
        lvLabel.color(Color.ofArgb(COLOR_LV));
        lvLabel.horizontalSizing(Sizing.fixed(70));
        root.child(lvLabel);

        // XP 进度条 —— 固定宽 120，内嵌 fill 以百分比宽度呈现。
        xpTrack = Containers.horizontalFlow(Sizing.fixed(120), Sizing.fixed(6));
        xpTrack.surface(Surface.flat(COLOR_XP_TRACK));
        xpTrack.horizontalAlignment(HorizontalAlignment.LEFT);
        xpFill = Containers.horizontalFlow(Sizing.fixed(0), Sizing.fixed(6));
        xpFill.surface(Surface.flat(COLOR_XP_FILL));
        xpTrack.child(xpFill);
        root.child(xpTrack);

        gainLabel = Components.label(Text.literal(""));
        gainLabel.color(Color.ofArgb(COLOR_GAIN));
        gainLabel.horizontalSizing(Sizing.fixed(60));
        root.child(gainLabel);
    }

    public SkillId skill() {
        return id;
    }

    public FlowLayout component() {
        return root;
    }

    /**
     * 刷新此行的所有字段。{@code nowMs} 由调用方传入（通常 {@link System#currentTimeMillis()}）
     * 用于"最近 +XP" 3s 窗口判定。
     */
    public void update(SkillSetSnapshot.Entry entry, long nowMs) {
        if (entry == null) entry = SkillSetSnapshot.Entry.zero();

        int effective = entry.effectiveLv();
        boolean capped = entry.lv() > entry.cap();
        // plan §5.1 "Lv.X / cap Y" 显示，超 cap 灰色提示
        lvLabel.text(Text.literal(
            "Lv." + entry.lv() + " / cap " + entry.cap()
                + (capped ? " (压制→" + effective + ")" : "")
        ));
        lvLabel.color(Color.ofArgb(capped ? COLOR_CAPPED : COLOR_LV));

        double ratio = entry.progressRatio();
        int fillW = Math.max(0, Math.min(120, (int) Math.round(ratio * 120.0)));
        xpFill.horizontalSizing(Sizing.fixed(fillW));

        if (entry.recentGainXp() > 0 && (nowMs - entry.recentGainMillis()) < RECENT_GAIN_WINDOW_MS) {
            gainLabel.text(Text.literal("+" + entry.recentGainXp()));
        } else {
            gainLabel.text(Text.literal(""));
        }
    }

    public void setSelected(boolean selected) {
        root.surface(selected
            ? Surface.flat(COLOR_BG_SELECTED).and(Surface.outline(COLOR_BORDER_SELECTED))
            : Surface.flat(COLOR_BG));
    }
}
