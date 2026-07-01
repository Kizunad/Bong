package com.bong.client.hud;

import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;

public enum HudLayoutPreset {
    COMBAT(EnumSet.of(
        Widget.QI_RADAR,
        Widget.COMPASS,
        Widget.THREAT,
        Widget.MINI_BODY,
        Widget.BARS,
        Widget.TARGET,
        Widget.EVENT_STREAM,
        Widget.PROCESSING,
        Widget.CRITICAL
    )),
    EXPLORATION(EnumSet.of(
        Widget.COMPASS,
        Widget.QI_RADAR,
        Widget.ZONE,
        Widget.BARS,
        Widget.EVENT_STREAM,
        Widget.PROCESSING,
        Widget.BOTANY,
        Widget.CRITICAL
    )),
    CULTIVATION(EnumSet.of(
        Widget.QI_RADAR,
        Widget.BARS,
        Widget.EVENT_STREAM,
        Widget.PROCESSING,
        Widget.MERIDIAN,
        Widget.LINGTIAN,
        Widget.CRITICAL
    ));

    public static final long HIDE_MS = 200L;
    public static final long SHOW_MS = 300L;
    public static final long SHOW_DELAY_MS = 100L;

    private final EnumSet<Widget> defaultWidgets;

    HudLayoutPreset(EnumSet<Widget> defaultWidgets) {
        this.defaultWidgets = defaultWidgets;
    }

    public EnumSet<Widget> defaultWidgets() {
        return EnumSet.copyOf(defaultWidgets);
    }

    public static HudLayoutPreset fromMode(HudImmersionMode.Mode mode) {
        return switch (mode == null ? HudImmersionMode.Mode.PEACE : mode) {
            case COMBAT -> COMBAT;
            case CULTIVATION -> CULTIVATION;
            case PEACE -> EXPLORATION;
        };
    }

    public static List<HudRenderCommand> filter(
        List<HudRenderCommand> commands,
        HudImmersionMode.Mode mode,
        HudLayoutPreferenceStore.Density density,
        long nowMillis
    ) {
        List<HudRenderCommand> baselineFiltered = HudImmersionMode.filter(commands, mode);
        if (baselineFiltered.isEmpty()) {
            return baselineFiltered;
        }
        HudLayoutPreferenceStore.Density effectiveDensity =
            density == null ? HudLayoutPreferenceStore.Density.STANDARD : density;
        if (effectiveDensity == HudLayoutPreferenceStore.Density.MAXIMUM) {
            return baselineFiltered;
        }
        EnumSet<Widget> widgets = effectiveDensity == HudLayoutPreferenceStore.Density.MINIMAL
            ? EnumSet.of(Widget.ZONE, Widget.BARS, Widget.EVENT_STREAM, Widget.CRITICAL)
            : HudLayoutPreferenceStore.widgetsFor(fromMode(mode));
        List<HudRenderCommand> out = new ArrayList<>(baselineFiltered.size());
        for (HudRenderCommand command : baselineFiltered) {
            Widget widget = widgetFor(command.layer());
            if (widgets.contains(widget) || widget == Widget.ALWAYS) {
                out.add(applyPresetAlpha(command, widget, nowMillis));
            }
        }
        return List.copyOf(out);
    }

    public static double alphaForWidget(boolean showing, long elapsedMillis) {
        long elapsed = Math.max(0L, elapsedMillis);
        if (!showing) {
            return 1.0 - Math.min(1.0, elapsed / (double) HIDE_MS);
        }
        if (elapsed <= SHOW_DELAY_MS) {
            return 0.0;
        }
        return Math.min(1.0, (elapsed - SHOW_DELAY_MS) / (double) SHOW_MS);
    }

    private static HudRenderCommand applyPresetAlpha(HudRenderCommand command, Widget widget, long nowMillis) {
        if (command == null || widget == Widget.ALWAYS || widget == Widget.CRITICAL) {
            return command;
        }
        double alpha = alphaForWidget(true, HudImmersionMode.transitionElapsedMillis(nowMillis));
        return HudCommandAlpha.withAlpha(command, alpha);
    }

    static Widget widgetFor(HudRenderLayer layer) {
        if (layer == null) {
            return Widget.ALWAYS;
        }
        return switch (layer) {
            case BASELINE -> Widget.ALWAYS;
            case ZONE, HUD_VARIANT -> Widget.ZONE;
            case COMPASS -> Widget.COMPASS;
            case QI_RADAR -> Widget.QI_RADAR;
            case THREAT_INDICATOR, EDGE_FEEDBACK, NEAR_DEATH, TRIBULATION -> Widget.THREAT;
            case MINI_BODY, STAMINA_BAR, DERIVED_ATTR, STATUS_EFFECTS, MOVEMENT_HUD -> Widget.BARS;
            case QUICK_BAR, CAST_BAR, SPELL_VOLUME, CARRIER, JIEMAI_RING, VORTEX_CHARGE, VORTEX_COOLDOWN,
                VORTEX_BACKFIRE, VORTEX_TURBULENCE, DUGU_TAINT_WARNING, DUGU_TAINT_INDICATOR,
                DUGU_REVEAL_RISK, DUGU_SELF_CURE_PROGRESS, DUGU_SHROUD, DUGU_QI_DECAY, POISON_TRAIT, COFFIN,
                DANDAO_MUTATION, SWORD_BOND,
                // plan-dying-elder-v1 P3：大能遭遇交互面板跟随 BARS 组
                DYING_ELDER -> Widget.BARS;
            case TARGET_INFO -> Widget.TARGET;
            // F5 fix — 灵龛守护面板与 NpcInteractionLogHudPlanner（TARGET_INFO）同组，
            // 都是"目标/状态类侧栏信息"，密度收紧时一起隐藏/显示。
            case NICHE_GUARDIAN -> Widget.TARGET;
            case EVENT_STREAM, TOAST -> Widget.EVENT_STREAM;
            case BOTANY -> Widget.BOTANY;
            case LINGTIAN_OVERLAY -> Widget.LINGTIAN;
            case PROCESSING_HUD, SEARCH_PROGRESS, TSY_EXTRACT, HOME_SEQUENCE, REALM_COLLAPSE, GATHERING -> Widget.PROCESSING;
            case MERIDIAN_OPEN -> Widget.MERIDIAN;
            case VISUAL, SPIRITUAL_SENSE, DAMAGE_FLOATER, FLIGHT_HUD, CONNECTION_STATUS,
                HALLUCINATION -> Widget.CRITICAL;
            case YIDAO -> Widget.CRITICAL;
            // plan-agent-ui-data-v1 P3：天道动态面板 VFX（fade-in/vignette/shake）归 CRITICAL（常驻）
            case AGENT_UI -> Widget.CRITICAL;
            // plan-halfstep-rechallenge-integration-v1 P0：重渡触发 HUD 归 CRITICAL（时限通知，常驻）
            case HALFSTEP_RECHALLENGE -> Widget.CRITICAL;
            // 震脉瞬态成功反馈（极限弹反成功 / 盾格挡命中）归 CRITICAL：仅在触发瞬间闪现，
            // 不受密度抑制——否则极简密度下玩家收不到「弹反/格挡成功」这一关键反馈。
            case ZHENMAI_PARRY, SHIELD_BLOCK -> Widget.CRITICAL;
            // 震脉持续/微指示（护脉减伤条 / 多点反震计数 / 局部中和提示 / 绝脉断链增幅倒计时条）归 BARS：
            // 与其他战斗 buff 微条同组，极简密度下随 BARS 一并接受密度 alpha 处理。
            // ZHENMAI_SEVER 是最长 60s 的增幅窗口倒计时条（与 ZHENMAI_HARDEN 同形），不是瞬态闪现，
            // 不能归 CRITICAL（否则极简密度下会满亮常驻整整一分钟）。
            case ZHENMAI_HARDEN, ZHENMAI_MULTIPOINT, ZHENMAI_NEUTRALIZE, ZHENMAI_SEVER -> Widget.BARS;
        };
    }

    public enum Widget {
        ALWAYS,
        ZONE,
        QI_RADAR,
        COMPASS,
        THREAT,
        MINI_BODY,
        BARS,
        TARGET,
        EVENT_STREAM,
        BOTANY,
        LINGTIAN,
        PROCESSING,
        MERIDIAN,
        CRITICAL
    }
}
