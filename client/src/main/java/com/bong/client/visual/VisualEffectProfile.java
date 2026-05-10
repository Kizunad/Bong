package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;

enum VisualEffectProfile {
    SYSTEM_WARNING(
        VisualEffectState.EffectType.SCREEN_SHAKE,
        0xF07C3E,
        0.85,
        2_400L,
        1_200L,
        255,
        "≋ 天道警示 ≋"
    ),
    PERCEPTION(
        VisualEffectState.EffectType.FOG_TINT,
        0x5F7693,
        0.55,
        4_500L,
        1_500L,
        96,
        null
    ),
    ERA_DECREE(
        VisualEffectState.EffectType.TITLE_FLASH,
        0xF2CC6B,
        0.75,
        3_200L,
        2_200L,
        255,
        "✦ 时代法旨 ✦"
    ),
    // 血月：全屏红染，夜间 + 血月事件触发；低 alpha 不遮挡视野，常态可长时间挂着
    BLOOD_MOON(
        VisualEffectState.EffectType.BLOOD_MOON,
        0xC01818,
        0.6,
        30_000L,
        3_000L,
        96,
        null
    ),
    // 入魔黑雾：边缘深红/黑 vignette，常态挂在玩家身上
    DEMONIC_FOG(
        VisualEffectState.EffectType.DEMONIC_FOG,
        0x440808,
        0.8,
        30_000L,
        3_000L,
        200,
        null
    ),
    // 顿悟金光：短暂全屏金白一闪，强 alpha + 快衰减
    ENLIGHTENMENT_FLASH(
        VisualEffectState.EffectType.ENLIGHTENMENT_FLASH,
        0xFFF2C0,
        1.0,
        1_500L,
        6_000L,
        255,
        null
    ),
    // 天劫压抑：全屏灰蓝 + 边缘收紧 vignette，临劫压迫感
    TRIBULATION_PRESSURE(
        VisualEffectState.EffectType.TRIBULATION_PRESSURE,
        0x2A3348,
        0.7,
        10_000L,
        2_000L,
        144,
        null
    ),
    // 运功 FOV 收缩：FOV 从默认收紧到更窄，给"专注/凝神"感；持续型，长 duration 等待主动 clear
    FOV_ZOOM_IN(
        VisualEffectState.EffectType.FOV_ZOOM_IN,
        0,
        1.0,
        30_000L,
        500L,
        0,
        null
    ),
    // 破境 FOV 拉伸：FOV 瞬时往外推，500ms 内回弹，制造"画面突然广角"的爆发感
    FOV_STRETCH(
        VisualEffectState.EffectType.FOV_STRETCH,
        0,
        1.0,
        500L,
        200L,
        0,
        null
    ),
    // 天劫仰视：镜头 pitch 单调上抬到 ~25°，钟形曲线自动回落，为天劫降临做视觉铺垫
    TRIBULATION_LOOK_UP(
        VisualEffectState.EffectType.TRIBULATION_LOOK_UP,
        0,
        1.0,
        4_000L,
        1_500L,
        0,
        null
    ),
    // 入定淡青：运功常态，淡翠青 tint + 边缘渐暗 vignette，低 alpha 不干扰视野
    MEDITATION_CALM(
        VisualEffectState.EffectType.MEDITATION_CALM,
        0x7FB8A0,
        0.5,
        20_000L,
        1_500L,
        72,
        null
    ),
    // 中毒酸绿：debuff 类，全屏酸绿淡染
    POISON_TINT(
        VisualEffectState.EffectType.POISON_TINT,
        0x4FB020,
        0.6,
        15_000L,
        1_000L,
        96,
        null
    ),
    // 寒毒冰蓝：冰蓝 tint + 边缘 vignette，当前版本以色彩近似结霜，结霜纹理留待资产到位
    FROSTBITE(
        VisualEffectState.EffectType.FROSTBITE,
        0x8FCFF0,
        0.7,
        15_000L,
        1_500L,
        110,
        null
    ),
    // 濒死视界：黑色 vignette 压缩视野，满强度起步随 duration 线性衰减（简化版，暂不做收紧动画）
    NEAR_DEATH_VIGNETTE(
        VisualEffectState.EffectType.NEAR_DEATH_VIGNETTE,
        0x000000,
        1.0,
        30_000L,
        1_000L,
        220,
        null
    ),
    // 灵压晃动：低幅低频相机抖动，营造"某种庞大东西正接近"的心跳/压迫感
    PRESSURE_JITTER(
        VisualEffectState.EffectType.PRESSURE_JITTER,
        0,
        0.7,
        6_000L,
        1_000L,
        0,
        null
    ),
    // 受创镜头后退：瞬时把相机沿 facing 反方向推，线性回弹；350ms 短 duration 配 150ms 重触发让连击可见
    HIT_PUSHBACK(
        VisualEffectState.EffectType.HIT_PUSHBACK,
        0,
        1.0,
        350L,
        150L,
        0,
        null
    ),
    WEAPON_BREAK_FLASH(
        VisualEffectState.EffectType.WEAPON_BREAK_FLASH,
        0xFF4040,
        1.0,
        260L,
        120L,
        160,
        null
    ),
    ARMOR_EQUIP_FLASH(
        VisualEffectState.EffectType.ARMOR_EQUIP_FLASH,
        0xFFFFFF,
        0.1,
        100L,
        60L,
        96,
        null
    ),
    ARMOR_LOW_DURABILITY_FLASH(
        VisualEffectState.EffectType.ARMOR_LOW_DURABILITY_FLASH,
        0xFF3030,
        1.0,
        700L,
        250L,
        112,
        null
    ),
    ARMOR_BREAK_FLASH(
        VisualEffectState.EffectType.ARMOR_BREAK_FLASH,
        0xFF3030,
        1.0,
        300L,
        120L,
        176,
        null
    ),
    // 水墨边框：入定/回忆常态，四角墨晕贴图（中心透明），替代 MEDITATION_CALM 的纯色 vignette 走沉浸视觉
    MEDITATION_INK_WASH(
        VisualEffectState.EffectType.MEDITATION_INK_WASH,
        0xFFFFFF,
        0.7,
        20_000L,
        1_500L,
        200,
        null
    );

    private final VisualEffectState.EffectType effectType;
    private final int baseColor;
    private final double maxIntensity;
    private final long maxDurationMillis;
    private final long retriggerWindowMillis;
    private final int maxAlpha;
    private final String overlayLabel;

    VisualEffectProfile(
        VisualEffectState.EffectType effectType,
        int baseColor,
        double maxIntensity,
        long maxDurationMillis,
        long retriggerWindowMillis,
        int maxAlpha,
        String overlayLabel
    ) {
        this.effectType = effectType;
        this.baseColor = baseColor;
        this.maxIntensity = maxIntensity;
        this.maxDurationMillis = maxDurationMillis;
        this.retriggerWindowMillis = retriggerWindowMillis;
        this.maxAlpha = maxAlpha;
        this.overlayLabel = overlayLabel;
    }

    static VisualEffectProfile from(VisualEffectState visualEffectState) {
        if (visualEffectState == null || visualEffectState.isEmpty()) {
            return null;
        }

        return switch (visualEffectState.effectType()) {
            case SCREEN_SHAKE -> SYSTEM_WARNING;
            case FOG_TINT -> PERCEPTION;
            case TITLE_FLASH -> ERA_DECREE;
            case BLOOD_MOON -> BLOOD_MOON;
            case DEMONIC_FOG -> DEMONIC_FOG;
            case ENLIGHTENMENT_FLASH -> ENLIGHTENMENT_FLASH;
            case TRIBULATION_PRESSURE -> TRIBULATION_PRESSURE;
            case FOV_ZOOM_IN -> FOV_ZOOM_IN;
            case FOV_STRETCH -> FOV_STRETCH;
            case TRIBULATION_LOOK_UP -> TRIBULATION_LOOK_UP;
            case MEDITATION_CALM -> MEDITATION_CALM;
            case POISON_TINT -> POISON_TINT;
            case FROSTBITE -> FROSTBITE;
            case NEAR_DEATH_VIGNETTE -> NEAR_DEATH_VIGNETTE;
            case PRESSURE_JITTER -> PRESSURE_JITTER;
            case HIT_PUSHBACK -> HIT_PUSHBACK;
            case WEAPON_BREAK_FLASH -> WEAPON_BREAK_FLASH;
            case ARMOR_EQUIP_FLASH -> ARMOR_EQUIP_FLASH;
            case ARMOR_LOW_DURABILITY_FLASH -> ARMOR_LOW_DURABILITY_FLASH;
            case ARMOR_BREAK_FLASH -> ARMOR_BREAK_FLASH;
            case MEDITATION_INK_WASH -> MEDITATION_INK_WASH;
            case NONE -> null;
        };
    }

    VisualEffectState.EffectType effectType() {
        return effectType;
    }

    int baseColor() {
        return baseColor;
    }

    double maxIntensity() {
        return maxIntensity;
    }

    long maxDurationMillis() {
        return maxDurationMillis;
    }

    long retriggerWindowMillis() {
        return retriggerWindowMillis;
    }

    int maxAlpha() {
        return maxAlpha;
    }

    String overlayLabel() {
        return overlayLabel;
    }
}
