package com.bong.client.social;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

/**
 * F5 fix — {@link NicheGuardianStore} 数据此前只进不出（只被 {@code NicheIntrusionAlertHandler}
 * 写入，从未被任何 HUD planner 读取渲染）。照 {@code com.bong.client.npc.NpcInteractionLogHudPlanner}
 * 先例：非空门控 + 固定宽度侧栏面板。
 *
 * <p>与 NpcInteractionLogHudPlanner 不同的是本面板没有显式的"是否打开"开关——玩家从未主动放置过
 * 守护载体、也没收到过龛侵警报时，store 完全为空，面板整体隐藏（HUD 条件显示约束：未激活隐藏而非
 * 灰置）。一旦 store 中出现任何守护状态或龛侵记录，面板即常驻显示直到状态被覆盖。
 */
public final class NicheGuardianHudPlanner {
    static final int WIDTH = 190;
    static final int ROW_HEIGHT = 12;
    static final int BG = 0xCC101018;
    static final int BORDER = 0xAAE8D090;
    static final int TEXT = 0xFFECE8D8;
    static final int WARN = 0xFFE06058;

    private NicheGuardianHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        boolean hasContent = !NicheGuardianStore.guardianStatuses().isEmpty()
            || !NicheGuardianStore.intrusionAlerts().isEmpty();
        if (!hasContent || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }

        List<String> lines = NicheGuardianPanel.buildLines();
        if (lines.isEmpty()) {
            return List.of();
        }

        int x = Math.max(8, screenWidth - WIDTH - 10);
        int y = Math.max(28, screenHeight / 2 - (lines.size() * ROW_HEIGHT) / 2);
        int height = 16 + lines.size() * ROW_HEIGHT;

        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.NICHE_GUARDIAN, x, y, WIDTH, height, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.NICHE_GUARDIAN, x, y, WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.NICHE_GUARDIAN, x, y + height - 1, WIDTH, 1, BORDER));
        out.add(HudRenderCommand.text(HudRenderLayer.NICHE_GUARDIAN, "灵龛守护", x + 6, y + 5, TEXT));

        int rowY = y + 18;
        for (String line : lines) {
            int color = line.startsWith("龛侵") || line.contains("broken") ? WARN : TEXT;
            out.add(HudRenderCommand.text(HudRenderLayer.NICHE_GUARDIAN, line, x + 6, rowY, color));
            rowY += ROW_HEIGHT;
        }
        return List.copyOf(out);
    }
}
