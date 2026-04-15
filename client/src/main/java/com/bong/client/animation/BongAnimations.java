package com.bong.client.animation;

import net.minecraft.util.Identifier;

/**
 * Bong 动画 id 常量表 + bootstrap 钩子。
 *
 * <p><b>Phase 2 全 JSON 化</b>：19 个 Phase 1 动画全部迁到 {@code assets/bong/player_animation/}
 * 下的 Emotecraft v3 JSON，由 PlayerAnimator 自带的 reload listener 自动加载，查询走
 * {@link BongAnimationRegistry#get}（JSON-first fallback Java, 实际 Java 源已空）。
 *
 * <p>JSON 生成器：{@code client/tools/gen_*.py}（参见 {@code anim_common.py} 的 emitter 约定）。
 * 迭代时修 Python 源码，跑 {@code python3 gen_xxx.py} 重写 JSON，F3+T 热重载即可查看。
 */
public final class BongAnimations {
    /** 所有 Bong 动画的命名空间。 */
    public static final String MOD_ID = "bong";

    // §5.1 战斗类
    public static final Identifier SWORD_SWING_HORIZ = new Identifier(MOD_ID, "sword_swing_horiz");
    public static final Identifier SWORD_SWING_VERT = new Identifier(MOD_ID, "sword_swing_vert");
    public static final Identifier SWORD_STAB = new Identifier(MOD_ID, "sword_stab");
    public static final Identifier FIST_PUNCH_RIGHT = new Identifier(MOD_ID, "fist_punch_right");
    public static final Identifier FIST_PUNCH_LEFT = new Identifier(MOD_ID, "fist_punch_left");
    public static final Identifier PALM_THRUST = new Identifier(MOD_ID, "palm_thrust");
    public static final Identifier GUARD_RAISE = new Identifier(MOD_ID, "guard_raise");
    public static final Identifier DODGE_BACK = new Identifier(MOD_ID, "dodge_back");
    public static final Identifier HIT_RECOIL = new Identifier(MOD_ID, "hit_recoil");

    // §5.2 修仙姿态
    public static final Identifier MEDITATE_SIT = new Identifier(MOD_ID, "meditate_sit");
    public static final Identifier CULTIVATE_STAND = new Identifier(MOD_ID, "cultivate_stand");
    public static final Identifier LEVITATE = new Identifier(MOD_ID, "levitate");
    public static final Identifier SWORD_RIDE = new Identifier(MOD_ID, "sword_ride");
    public static final Identifier CAST_INVOKE = new Identifier(MOD_ID, "cast_invoke");
    public static final Identifier RUNE_DRAW = new Identifier(MOD_ID, "rune_draw");

    // §5.3 剧情演绎
    public static final Identifier BREAKTHROUGH_BURST = new Identifier(MOD_ID, "breakthrough_burst");
    public static final Identifier TRIBULATION_BRACE = new Identifier(MOD_ID, "tribulation_brace");
    public static final Identifier ENLIGHTENMENT_POSE = new Identifier(MOD_ID, "enlightenment_pose");
    public static final Identifier DEATH_COLLAPSE = new Identifier(MOD_ID, "death_collapse");
    public static final Identifier BOW_SALUTE = new Identifier(MOD_ID, "bow_salute");

    private BongAnimations() {
    }

    /**
     * 客户端启动钩子。
     *
     * <p>JSON 加载由 {@link dev.kosmx.playerAnim.minecraftApi.PlayerAnimationRegistry} 的
     * Fabric resource reload listener 自动处理，这里不再显式 register。调用点保留是为了
     * 给未来可能的自举逻辑（如：启动时做一次 JSON 可用性检查、日志计数等）留入口。
     */
    public static void bootstrap() {
        // 全部 JSON 化后此处无需显式 register —— 保留空方法做为启动钩子。
    }
}
