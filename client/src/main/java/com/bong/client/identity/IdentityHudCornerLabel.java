package com.bong.client.identity;

import java.util.List;
import java.util.Locale;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;

/**
 * HUD 角标（plan-identity-v1 §7）—— 屏幕右下角显示当前 active identity。
 *
 * <p>玩家自己看 ≠ §K 红线（worldview §K：节律红线管"汐转 / 季节"完全不显式；identity 玩家
 * 自己要知道当下装在哪个面具，必须显式 HUD 展示）。**NPC 反应分级（High/Normal/Low/Wanted）
 * 不显式**——玩家通过 NPC 反应自己悟。
 *
 * <p>渲染依赖 {@link IdentityPanelStateStore}：server CustomPayload 推送
 * {@code bong:identity_panel_state} 后，store 替换 snapshot，下一次 HUD tick
 * {@link #buildCommands(HudTextHelper.WidthMeasurer, int)} 读 snapshot 输出 label 命令。
 */
public final class IdentityHudCornerLabel {
    private static final int RIGHT_PADDING = 4;
    private static final int Y = 4;
    private static final int COLOR_NORMAL = 0xFFFFFFFF;
    private static final int COLOR_FROZEN = 0xFFA0A0A0;

    private IdentityHudCornerLabel() {}

    /**
     * @param widthMeasurer  字体宽度测量器（HUD 父容器提供）
     * @param screenWidth    HUD 视口宽度
     * @return HUD 命令；当前 identity 不可用时返回空
     */
    public static List<HudRenderCommand> buildCommands(
            HudTextHelper.WidthMeasurer widthMeasurer,
            int screenWidth) {
        IdentityPanelState state = IdentityPanelStateStore.snapshot();
        if (state.identities().isEmpty()) {
            return List.of();
        }
        IdentityPanelEntry active = state.activeEntry().orElse(null);
        if (active == null) {
            return List.of();
        }
        String label = formatLabel(active);
        int color = active.frozen() ? COLOR_FROZEN : COLOR_NORMAL;
        int textWidth = widthMeasurer == null ? label.length() * 6 : widthMeasurer.measure(label);
        int x = screenWidth - textWidth - RIGHT_PADDING;
        if (x < 0) {
            x = 0;
        }
        return List.of(HudRenderCommand.text(HudRenderLayer.BASELINE, label, x, Y, color));
    }

    static String formatLabel(IdentityPanelEntry entry) {
        if (entry.frozen()) {
            return String.format(Locale.ROOT, "[#%d] %s [冷藏]", entry.identityId(), entry.displayName());
        }
        return String.format(Locale.ROOT, "[#%d] %s", entry.identityId(), entry.displayName());
    }
}
