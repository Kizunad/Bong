package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;

import java.util.ArrayList;
import java.util.List;

public final class NearDeathCollapsePlanner {
    static final int QI_COLOR = 0x88C84444;
    static final int MERIDIAN_COLOR = 0xB0FF4040;
    static final int SURFACE_COLOR = 0xAA5A0000;
    // F15 fix — collapseFreezeBeforeDeath() 窗口内的"冻结帧"用固定满强度 alpha，
    // 窗口外沿用原来渐进的 alpha 常量。不改 qiEscapeDensityByHp/surfaceCrackLines/
    // meridianGlowOnSevered/collapseFreezeBeforeDeath 四个纯函数本身的触发阈值。
    static final int FROZEN_ALPHA = 255;
    static final int ESCAPE_ALPHA = 140;
    static final int CRACK_ALPHA = 200;

    private NearDeathCollapsePlanner() {}

    public static List<HudRenderCommand> buildCommands(DeathCinematicState state, int width, int height) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active() || width <= 0 || height <= 0) return out;
        double progress = state.phaseProgress();
        // F15 fix — predeath 阶段没有真实 hpPercent 字段（DeathCinematicState 只携带
        // phase/phaseTick/roll/insightText 等，无 hp/severed-meridian 信号）。用阶段推进度
        // 作为"濒死血线下沉"的代理信号：刚进入 predeath 时 hp≈1.0，临近 death_moment 时 hp≈0。
        double hpPercent = Math.max(0.0, 1.0 - progress);
        boolean frozen = collapseFreezeBeforeDeath(state.phaseTick());

        out.add(HudRenderCommand.screenTint(
            HudRenderLayer.NEAR_DEATH,
            HudTextHelper.withAlpha(QI_COLOR, 40 + (int) (progress * 60))
        ));

        // meridianGlowOnSevered 需要一个"经脉已断"信号，state 没有直接字段；复用 finalDeath()——
        // 本条命数已是最终死亡时，经脉视为不可逆断裂，与 hp 阈值触发共享同一条 edgeVignette。
        boolean severedGlow = meridianGlowOnSevered(state.finalDeath(), hpPercent);
        out.add(HudRenderCommand.edgeVignette(
            HudRenderLayer.NEAR_DEATH,
            HudTextHelper.withAlpha(severedGlow ? MERIDIAN_COLOR : QI_COLOR, 80 + (int) (progress * 70))
        ));

        int escapeCount = qiEscapeDensityByHp(hpPercent);
        int escapeAlpha = frozen ? FROZEN_ALPHA : ESCAPE_ALPHA;
        for (int i = 0; i < escapeCount; i++) {
            int spread = 16 + i * 14;
            out.add(HudRenderCommand.rect(
                HudRenderLayer.NEAR_DEATH,
                Math.max(2, width / 2 - spread),
                Math.max(2, height / 2 - spread / 2),
                3,
                spread,
                HudTextHelper.withAlpha(QI_COLOR, escapeAlpha)
            ));
        }

        int crackCount = surfaceCrackLines(hpPercent);
        int crackAlpha = frozen ? FROZEN_ALPHA : CRACK_ALPHA;
        for (int i = 0; i < crackCount; i++) {
            int lineX = Math.max(2, (width * (i + 1)) / (crackCount + 1));
            out.add(HudRenderCommand.rect(
                HudRenderLayer.NEAR_DEATH,
                lineX,
                Math.max(2, height / 2 - 24),
                1,
                48,
                HudTextHelper.withAlpha(SURFACE_COLOR, crackAlpha)
            ));
        }

        out.add(HudRenderCommand.text(
            HudRenderLayer.NEAR_DEATH,
            state.skipPredeath() ? "你已经习惯死亡" : "真元外泄 · 经脉承压 · 肉身将裂",
            Math.max(8, width / 2 - 76),
            height - 46,
            0xFFFFC0C0
        ));
        return out;
    }

    public static int qiEscapeDensityByHp(double hpPercent) {
        double clamped = Math.max(0.0, Math.min(1.0, hpPercent));
        if (clamped >= 0.20) return 0;
        return Math.max(1, (int) Math.ceil((1.0 - clamped) * 3.0));
    }

    public static boolean meridianGlowOnSevered(boolean hasSeveredMeridian, double hpPercent) {
        return hasSeveredMeridian || hpPercent < 0.10;
    }

    public static int surfaceCrackLines(double hpPercent) {
        return hpPercent < 0.05 ? 8 : 0;
    }

    public static boolean collapseFreezeBeforeDeath(long phaseTick) {
        return phaseTick >= 14L && phaseTick <= 20L;
    }
}
