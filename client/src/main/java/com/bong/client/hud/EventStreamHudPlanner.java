package com.bong.client.hud;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStream;

import java.util.ArrayList;
import java.util.List;

/**
 * Right-side unified event stream (§2.3). Renders a compact one-line-per-event
 * list anchored to the upper right of the screen.
 */
public final class EventStreamHudPlanner {
    public static final int PANEL_WIDTH = 200;
    public static final int LINE_HEIGHT = 10;
    public static final int RIGHT_MARGIN = 8;
    public static final int TOP_MARGIN = 80;
    public static final int BG_COLOR = 0x60000000;
    public static final int MAX_VISIBLE = 18;

    private EventStreamHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        UnifiedEventStream stream,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (stream == null || screenWidth <= 0 || screenHeight <= 0) return out;

        // Drop expired events before reading the snapshot.
        stream.expire(nowMillis);
        List<UnifiedEvent> entries = stream.snapshot();
        if (entries.isEmpty()) return out;

        int x = screenWidth - PANEL_WIDTH - RIGHT_MARGIN;
        int y = TOP_MARGIN;
        int visible = Math.min(MAX_VISIBLE, entries.size());
        int panelHeight = visible * LINE_HEIGHT + 4;
        out.add(HudRenderCommand.rect(HudRenderLayer.EVENT_STREAM, x, y, PANEL_WIDTH, panelHeight, BG_COLOR));

        for (int i = 0; i < visible; i++) {
            UnifiedEvent e = entries.get(i);
            String text = e.channel().icon() + " " + e.displayText();
            String clipped = HudTextHelper.clipToWidth(text, PANEL_WIDTH - 6, widthMeasurer);
            if (clipped == null || clipped.isEmpty()) continue;
            out.add(HudRenderCommand.text(
                HudRenderLayer.EVENT_STREAM,
                clipped,
                x + 4,
                y + 2 + i * LINE_HEIGHT,
                e.color() == 0 ? e.channel().defaultColor() : e.color()
            ));
        }

        return out;
    }
}
