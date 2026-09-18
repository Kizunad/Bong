package com.bong.client.practice;

import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.*;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

/** 工作台一致的墨绿底、暖金标记与细线层次。 */
final class PracticeStyle {
    static final int INK = 0xFF111D22, TEXT = 0xFFD8DDD1, MUTED = 0xFF8EAAA8, GOLD = 0xFFD5B879;
    private PracticeStyle() {}

    static FlowLayout column() {
        var flow = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        flow.gap(7);
        return flow;
    }

    static LabelComponent label(String value, int color) {
        var label = Components.label(Text.literal(value));
        label.color(Color.ofArgb(color));
        label.horizontalSizing(Sizing.fill(100));
        return label;
    }

    static FlowLayout section(String title) {
        var flow = column();
        flow.padding(Insets.of(9));
        flow.surface(Surface.flat(INK).and(Surface.outline(0xFF33474A)));
        flow.child(label(title, GOLD));
        return flow;
    }

    static LabelComponent link(String text, Runnable action) {
        var link = label(text, GOLD);
        link.cursorStyle(CursorStyle.HAND);
        link.mouseDown().subscribe((x, y, button) -> { if (button != 0) return false; action.run(); return true; });
        return link;
    }

    static ButtonComponent button(String text, Runnable action) {
        var button = Components.button(Text.literal(text), ignored -> action.run());
        button.sizing(Sizing.fill(100), Sizing.fixed(24));
        button.textShadow(false);
        button.renderer(ButtonComponent.Renderer.flat(0xFF304347, 0xFF4C6464, 0xFF202D30));
        return button;
    }

    static FlowLayout value(String type, String value) {
        var row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.child(label(type, MUTED).horizontalSizing(Sizing.fill(38)));
        row.child(label(value, TEXT).horizontalSizing(Sizing.fill(62)));
        return row;
    }

    static final class Progress extends BaseComponent {
        private final double ratio;
        private final String caption;
        Progress(double ratio, String caption) {
            this.ratio = Double.isFinite(ratio) ? Math.max(0, Math.min(1, ratio)) : 0;
            this.caption = caption;
            sizing(Sizing.fill(100), Sizing.fixed(28));
        }
        @Override public void draw(OwoUIDrawContext ctx, int mx, int my, float partial, float delta) {
            ctx.fill(x, y + 19, x + width, y + 25, 0xFF0B1418);
            int end = x + (int) Math.round(width * ratio);
            ctx.fillGradient(x, y + 19, end, y + 25, 0xFF729B93, GOLD);
            for (int i = 1; i < 10; i++) ctx.fill(x + width * i / 10, y + 19, x + width * i / 10 + 1, y + 25, 0x88304747);
            var font = MinecraftClient.getInstance().textRenderer;
            ctx.drawText(font, font.trimToWidth(caption, width), x, y + 3, TEXT, false);
        }
    }
}
