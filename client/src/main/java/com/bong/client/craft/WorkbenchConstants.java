package com.bong.client.craft;

/**
 * plan-workbench-recipes-v1 P3.3 -- 制作台音效/VFX 规格常量。
 *
 * <p>放置/拆除/打开三项为 audio_recipe id，由 plan-workbench-place-runtime-v1 P2 接线。
 * 制作过程 4 个 SFX 与 3 组 VFX 仍归 plan-workbench-recipes-v1 消费。</p>
 */
public final class WorkbenchConstants {
    private WorkbenchConstants() {}

    // ========== SFX 常量 ==========

    /** 放置制作台：木块重落 + 铁钉微响。pitch 0.9-1.1, volume 0.8。 */
    public static final String SFX_PLACE = "workbench_place";

    /** 拆除制作台：木板裂响。pitch 0.8-1.0, volume 0.7。 */
    public static final String SFX_BREAK = "workbench_break";

    /** 打开制作台：木盖推开声。pitch 1.0-1.2, volume 0.5。 */
    public static final String SFX_OPEN = "workbench_open";

    // 以下 4 条为制作 session 视听，非本 plan 范围。
    /** 开始制作：工具碰撞声。pitch 0.9-1.1, volume 0.6。 */
    public static final String SFX_CRAFT_START = "bong:craft.workbench.start";

    /** 制作进行中：每 40 tick 一次轻响。pitch 0.8-1.2, volume 0.3。 */
    public static final String SFX_CRAFT_TICK = "bong:craft.workbench.tick";

    /** 制作完成：清脆落物声 + 微光音。pitch 1.0, volume 0.7。 */
    public static final String SFX_CRAFT_DONE = "bong:craft.workbench.done";

    /** 制作失败：沉闷断裂声。pitch 0.7, volume 0.5。 */
    public static final String SFX_CRAFT_FAIL = "bong:craft.workbench.fail";

    // ========== VFX 视觉效果规格常量 (3 组，制作 session 视听，非本 plan 范围) ==========

    // --- 制作进行中 VFX ---
    /** 粒子颜色：丹砂红 #8B3A3A。 */
    public static final int VFX_CINNABAR_COLOR = 0x8B3A3A;

    /** 制作进行中：粒子频率从 60 tick/次加速到 10 tick/次。 */
    public static final int VFX_IDLE_BURST_INTERVAL = 60;
    /** 制作进行中最大加速频率。 */
    public static final int VFX_ACTIVE_BURST_INTERVAL = 10;
    /** 空闲 burst 粒子数。 */
    public static final int VFX_IDLE_BURST_COUNT = 3;
    /** 制作中 burst 粒子数。 */
    public static final int VFX_ACTIVE_BURST_COUNT = 8;
    /** 空闲 burst 粒子 lifetime (tick)。 */
    public static final int VFX_IDLE_PARTICLE_LIFETIME = 10;
    /** 制作中 burst 粒子 lifetime (tick)。 */
    public static final int VFX_ACTIVE_PARTICLE_LIFETIME = 15;

    // --- 制作完成 VFX ---
    /** 完成 burst 粒子数。 */
    public static final int VFX_COMPLETE_BURST_COUNT = 12;
    /** 完成 burst 粒子 lifetime (tick)。 */
    public static final int VFX_COMPLETE_PARTICLE_LIFETIME = 20;
    /** 产出物品 3D icon 上浮高度 (block)。 */
    public static final float VFX_COMPLETE_ICON_RISE = 0.5f;

    // --- 真元消耗 VFX (qi_cost > 0 时) ---
    /** 灵气丝颜色：蓝白 #A8C4E0。 */
    public static final int VFX_QI_BEAM_COLOR = 0xA8C4E0;
    /** BongBeamParticle 宽度 (px)。 */
    public static final int VFX_QI_BEAM_WIDTH = 1;
    /** BongBeamParticle lifetime (tick)。 */
    public static final int VFX_QI_BEAM_LIFETIME = 30;
}
