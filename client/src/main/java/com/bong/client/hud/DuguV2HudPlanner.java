package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** Aggregates Dugu v2 HUD surfaces without owning gameplay state. */
public final class DuguV2HudPlanner {
    private static final int BAR_WIDTH = 92;
    private static final int BAR_HEIGHT = 5;

    private DuguV2HudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        DuguV2HudStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        DuguV2HudStateStore.State safeState = state == null ? DuguV2HudStateStore.State.NONE : state;
        if (screenWidth <= 0 || screenHeight <= 0) return List.of();

        List<HudRenderCommand> out = new ArrayList<>();
        if (safeState.tainted()) {
            out.add(HudRenderCommand.edgeVignette(HudRenderLayer.DUGU_TAINT_WARNING, 0x66305018));
            String hint = safeState.taintHint().isEmpty() ? "蛊毒" : safeState.taintHint();
            out.add(HudRenderCommand.text(
                HudRenderLayer.DUGU_TAINT_INDICATOR,
                hint,
                12,
                Math.max(12, screenHeight / 2 - 18),
                0x9BE15D
            ));
        }
        if (safeState.revealRisk() > 0f) {
            int x = Math.max(8, screenWidth - BAR_WIDTH - 12);
            int y = Math.max(12, screenHeight / 2 + 10);
            int filled = Math.max(1, Math.round(BAR_WIDTH * safeState.revealRisk()));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_REVEAL_RISK, x, y, BAR_WIDTH, BAR_HEIGHT, 0x44202020));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_REVEAL_RISK, x, y, filled, BAR_HEIGHT, 0xAA7FD447));
            out.add(HudRenderCommand.text(
                HudRenderLayer.DUGU_REVEAL_RISK,
                String.format(Locale.ROOT, "暴露 %.0f%%", safeState.revealRisk() * 100f),
                x,
                y - 10,
                0xC8F59A
            ));
        }
        if (safeState.selfCurePercent() > 0f || safeState.selfRevealed()) {
            int x = 12;
            int y = Math.max(24, screenHeight - 72);
            int filled = Math.max(1, Math.round(BAR_WIDTH * safeState.selfCurePercent() / 100f));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_SELF_CURE_PROGRESS, x, y, BAR_WIDTH, BAR_HEIGHT, 0x44202020));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_SELF_CURE_PROGRESS, x, y, filled, BAR_HEIGHT, 0xAA4A8F2A));
            String suffix = safeState.selfRevealed() ? " 已露" : "";
            out.add(HudRenderCommand.text(
                HudRenderLayer.DUGU_SELF_CURE_PROGRESS,
                String.format(Locale.ROOT, "自蕴 %.1f%%%s", safeState.selfCurePercent(), suffix),
                x,
                y - 10,
                0xBCEB88
            ));
        }
        if (safeState.shroudActive() && safeState.shroudUntilMs() > nowMillis) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.DUGU_SHROUD, 0x20224516));
        }
        // plan-combat-skill-feedback-bridges-v1 P5：永久真元上限衰减闪烁条。
        // 闪烁窗口内（decayExpiryMs > nowMillis）且 qiMaxDecayLoss > 0 时显示。
        // §视听精度文档待人工补（颜色/层级由此定，人工可后续调参）
        if (safeState.qiMaxDecayLoss() > 0f && safeState.decayExpiryMs() > nowMillis) {
            int x = Math.max(8, screenWidth / 2 - BAR_WIDTH / 2);
            int y = Math.max(8, screenHeight / 2 - 30);
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_QI_DECAY, x, y, BAR_WIDTH, BAR_HEIGHT, 0x44202020));
            // 衰减量占原上限（loss / (loss + qiMaxAfter)）比例着色，最低 1px
            float totalBefore = safeState.qiMaxDecayLoss() + safeState.qiMaxAfter();
            float ratio = totalBefore > 0f ? Math.min(1f, safeState.qiMaxDecayLoss() / totalBefore) : 0f;
            int filled = Math.max(1, Math.round(BAR_WIDTH * ratio));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_QI_DECAY, x, y, filled, BAR_HEIGHT, 0xCCB84B4B));
            out.add(HudRenderCommand.text(
                HudRenderLayer.DUGU_QI_DECAY,
                String.format(Locale.ROOT, "真元上限 -%.1f → %.1f", safeState.qiMaxDecayLoss(), safeState.qiMaxAfter()),
                x,
                y - 10,
                0xF0A07A
            ));
        }
        return List.copyOf(out);
    }
}
