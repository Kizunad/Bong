package com.bong.client.agentui;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import org.jetbrains.annotations.Nullable;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-agent-ui-data-v1 P3 — 天道动态面板 VFX HUD 命令生成器。
 *
 * <p>根据 {@link AgentUiVfxState} 生成逐帧 HUD 覆层命令：
 * <ul>
 *   <li>通用：fade-in 期间渐变 SCREEN_TINT alpha（从全黑→0透明），模拟面板淡入</li>
 *   <li>天道启示特有：EDGE_VIGNETTE 暗蓝（{@code #1A0D40}，opacity 0.4）持续 200t</li>
 *   <li>天道启示特有：EDGE_FEEDBACK 矩形模拟 0.5px shake（振动幅度极小）持续 40t</li>
 * </ul>
 *
 * <p>所有计算均为纯函数，输入 {@link AgentUiVfxState} + nowMs，无副作用。
 */
public final class AgentUiVfxPlanner {
    private AgentUiVfxPlanner() {}

    /**
     * 生成当前帧的 VFX HudRenderCommand 列表。
     *
     * @param state      当前 VFX 状态（null 返回空列表）
     * @param nowMs      当前时间戳（ms）
     * @param screenWidth  屏幕宽度（像素）
     * @param screenHeight 屏幕高度（像素）
     * @return 本帧应渲染的 HudRenderCommand 列表（可为空）
     */
    public static List<HudRenderCommand> buildCommands(
        @Nullable AgentUiVfxState state,
        long nowMs,
        int screenWidth,
        int screenHeight
    ) {
        if (state == null) {
            return List.of();
        }
        List<HudRenderCommand> out = new ArrayList<>();

        // ── fade-in：使用 SCREEN_TINT 从全黑渐淡（alpha = (1 - progress) * 0xFF）
        double fadeProgress = state.fadeInProgress(nowMs);
        if (fadeProgress < 1.0) {
            int alpha = (int) Math.round((1.0 - fadeProgress) * 0xFF);
            // SCREEN_TINT: ARGB, RGB=0 (black), alpha 从 0xFF→0
            int tintArgb = (alpha << 24) | 0x000000;
            out.add(HudRenderCommand.screenTint(HudRenderLayer.AGENT_UI, tintArgb));
        }

        // ── 天道启示：暗蓝 vignette（#1A0D40，opacity 0.4）持续 200t
        if (state.tiandaoVignetteActive(nowMs)) {
            out.add(HudRenderCommand.edgeVignette(
                HudRenderLayer.AGENT_UI,
                AgentUiVfxState.TIANDAO_VIGNETTE_ARGB
            ));
        }

        // ── 天道启示：轻微 shake（前 40t 内，用上下各 1px 的矩形条模拟抖动）
        if (state.tiandaoShakeActive(nowMs)) {
            long elapsed = nowMs - state.openedAtMs();
            // frequency 0.1Hz = 100ms 周期；用 sin 驱动幅度 0~1 映射到 1px alpha
            double phase = (elapsed / 100.0) * Math.PI * 2.0; // 0.1Hz = 2π / 1000ms × 100ms
            double shakeIntensity = Math.abs(Math.sin(phase)); // 0~1

            if (shakeIntensity > 0.01) {
                // amplitude 0.5px → 半透明水平条（最大 0.5 × 0xFF = 127 alpha）
                int shakeAlpha = (int) Math.round(shakeIntensity * 0.5 * 0xFF);
                int shakeColor = (shakeAlpha << 24) | AgentUiVfxState.TIANDAO_VIGNETTE_RGB;
                if (screenHeight > 1) {
                    // 顶部 1px
                    out.add(HudRenderCommand.rect(
                        HudRenderLayer.AGENT_UI, 0, 0, screenWidth, 1, shakeColor
                    ));
                    // 底部 1px
                    out.add(HudRenderCommand.rect(
                        HudRenderLayer.AGENT_UI, 0, screenHeight - 1, screenWidth, 1, shakeColor
                    ));
                }
            }
        }

        return out;
    }
}
