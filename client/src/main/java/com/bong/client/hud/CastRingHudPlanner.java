package com.bong.client.hud;

import com.bong.client.combat.CastState;

import java.util.ArrayList;
import java.util.List;

/** 施法残环：读取既有 CastState，弧段按进度显现，终态由 store 的原生命周期收尾。 */
public final class CastRingHudPlanner {
    static final List<String> SEGMENT_KEYS = List.of(
        "segment-a", "segment-b", "segment-c", "segment-d", "segment-e", "segment-f",
        "segment-g", "segment-h", "segment-i", "segment-j", "segment-k", "segment-l"
    );
    static final int SIZE = 44;

    private CastRingHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(CastState cast, long nowMs, int screenWidth, int screenHeight) {
        if (cast == null || cast.isIdle() || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        int barWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        int upperY = screenHeight - QuickBarHudPlanner.LOWER_BOTTOM_MARGIN
            - 2 * QuickBarHudPlanner.SLOT_SIZE - QuickBarHudPlanner.UPPER_GAP;
        // 放在快捷栏右上方，避开中央准星、截脉环及两侧武器槽。
        int x = Math.max(0, Math.min(screenWidth - SIZE - 6, (screenWidth + barWidth) / 2 - SIZE));
        int y = Math.max(0, upperY - SIZE - 6);
        List<HudRenderCommand> out = new ArrayList<>();
        if (cast.isCasting()) {
            float progress = cast.progress(nowMs);
            int size = Math.min(Math.min(screenWidth, screenHeight), Math.round(SIZE * (1.0f - 0.18f * progress)));
            int ringX = x + (SIZE - size) / 2;
            int ringY = y + (SIZE - size) / 2;
            out.add(HudRenderCommand.svg(HudRenderLayer.CAST_BAR, "track", ringX, ringY, size, size, 0xFFFFFFFF));
            for (int i = 0; i < SEGMENT_KEYS.size(); i++) {
                float visibility = Math.max(0.0f, Math.min(1.0f, progress * SEGMENT_KEYS.size() - i));
                int alpha = Math.round(255 * visibility);
                if (alpha > 0) {
                    out.add(HudRenderCommand.svg(HudRenderLayer.CAST_BAR, SEGMENT_KEYS.get(i),
                        ringX, ringY, size, size, (alpha << 24) | 0xFFFFFF));
                }
            }
        } else {
            String asset = cast.phase() == CastState.Phase.COMPLETE ? "complete" : "interrupted";
            out.add(HudRenderCommand.svg(HudRenderLayer.CAST_BAR, asset, x, y, SIZE, SIZE, 0xFFFFFFFF));
        }
        return List.copyOf(out);
    }
}
