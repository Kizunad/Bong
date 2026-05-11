package com.bong.client.animation;

import net.minecraft.util.Identifier;

import java.util.List;

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
    public static final Identifier SWORD_SWING_RIGHT = new Identifier(MOD_ID, "sword_swing_right");
    public static final Identifier SWORD_SWING_VERT = new Identifier(MOD_ID, "sword_swing_vert");
    public static final Identifier SWORD_SLASH_DOWN = new Identifier(MOD_ID, "sword_slash_down");
    public static final Identifier SWORD_STAB = new Identifier(MOD_ID, "sword_stab");
    public static final Identifier FIST_PUNCH_RIGHT = new Identifier(MOD_ID, "fist_punch_right");
    public static final Identifier FIST_PUNCH_LEFT = new Identifier(MOD_ID, "fist_punch_left");
    public static final Identifier PALM_THRUST = new Identifier(MOD_ID, "palm_thrust");
    public static final Identifier PALM_STRIKE = new Identifier(MOD_ID, "palm_strike");
    public static final Identifier GUARD_RAISE = new Identifier(MOD_ID, "guard_raise");
    public static final Identifier PARRY_BLOCK = new Identifier(MOD_ID, "parry_block");
    public static final Identifier DODGE_BACK = new Identifier(MOD_ID, "dodge_back");
    public static final Identifier DODGE_ROLL = new Identifier(MOD_ID, "dodge_roll");
    public static final Identifier HIT_RECOIL = new Identifier(MOD_ID, "hit_recoil");
    public static final Identifier HURT_STAGGER = new Identifier(MOD_ID, "hurt_stagger");
    public static final Identifier WINDUP_CHARGE = new Identifier(MOD_ID, "windup_charge");
    public static final Identifier RELEASE_BURST = new Identifier(MOD_ID, "release_burst");

    // §5.2 修仙姿态
    public static final Identifier MEDITATE_SIT = new Identifier(MOD_ID, "meditate_sit");
    public static final Identifier CULTIVATE_STAND = new Identifier(MOD_ID, "cultivate_stand");
    public static final Identifier LEVITATE = new Identifier(MOD_ID, "levitate");
    public static final Identifier SWORD_RIDE = new Identifier(MOD_ID, "sword_ride");
    public static final Identifier CAST_INVOKE = new Identifier(MOD_ID, "cast_invoke");
    public static final Identifier RUNE_DRAW = new Identifier(MOD_ID, "rune_draw");
    public static final Identifier EAT_FOOD = new Identifier(MOD_ID, "eat_food");
    public static final Identifier HARVEST_CROUCH = new Identifier(MOD_ID, "harvest_crouch");
    public static final Identifier LOOT_BEND = new Identifier(MOD_ID, "loot_bend");
    public static final Identifier STEALTH_CROUCH = new Identifier(MOD_ID, "stealth_crouch");
    public static final Identifier IDLE_BREATHE = new Identifier(MOD_ID, "idle_breathe");
    public static final Identifier TUIKE_DON_SKIN = new Identifier(MOD_ID, "tuike_don_skin");
    public static final Identifier TUIKE_SHED_BURST = new Identifier(MOD_ID, "tuike_shed_burst");
    public static final Identifier TUIKE_TAINT_TRANSFER = new Identifier(MOD_ID, "tuike_taint_transfer");

    // §5.3 剧情演绎
    public static final Identifier BREAKTHROUGH_BURST = new Identifier(MOD_ID, "breakthrough_burst");
    public static final Identifier BREAKTHROUGH_YINQI = new Identifier(MOD_ID, "breakthrough_yinqi");
    public static final Identifier BREAKTHROUGH_NINGMAI = new Identifier(MOD_ID, "breakthrough_ningmai");
    public static final Identifier BREAKTHROUGH_GUYUAN = new Identifier(MOD_ID, "breakthrough_guyuan");
    public static final Identifier BREAKTHROUGH_TONGLING = new Identifier(MOD_ID, "breakthrough_tongling");
    public static final Identifier TRIBULATION_BRACE = new Identifier(MOD_ID, "tribulation_brace");
    public static final Identifier ENLIGHTENMENT_POSE = new Identifier(MOD_ID, "enlightenment_pose");
    public static final Identifier DEATH_COLLAPSE = new Identifier(MOD_ID, "death_collapse");
    public static final Identifier DEATH_DISINTEGRATE = new Identifier(MOD_ID, "death_disintegrate");
    public static final Identifier REBIRTH_WAKE = new Identifier(MOD_ID, "rebirth_wake");
    public static final Identifier BOW_SALUTE = new Identifier(MOD_ID, "bow_salute");

    // plan-player-animation-implementation-v1：NPC / 产出交互 / 流派 stance / 状态步态
    public static final Identifier NPC_PATROL_WALK = new Identifier(MOD_ID, "npc_patrol_walk");
    public static final Identifier NPC_CHOP_TREE = new Identifier(MOD_ID, "npc_chop_tree");
    public static final Identifier NPC_MINE = new Identifier(MOD_ID, "npc_mine");
    public static final Identifier NPC_CROUCH_WAVE = new Identifier(MOD_ID, "npc_crouch_wave");
    public static final Identifier NPC_FLEE_RUN = new Identifier(MOD_ID, "npc_flee_run");
    public static final Identifier FORGE_HAMMER = new Identifier(MOD_ID, "forge_hammer");
    public static final Identifier ALCHEMY_STIR = new Identifier(MOD_ID, "alchemy_stir");
    public static final Identifier LINGTIAN_TILL = new Identifier(MOD_ID, "lingtian_till");
    public static final Identifier INVENTORY_REACH = new Identifier(MOD_ID, "inventory_reach");
    public static final Identifier STANCE_BAOMAI = new Identifier(MOD_ID, "stance_baomai");
    public static final Identifier STANCE_DUGU = new Identifier(MOD_ID, "stance_dugu");
    public static final Identifier STANCE_ZHENFA = new Identifier(MOD_ID, "stance_zhenfa");
    public static final Identifier STANCE_DUGU_POISON = new Identifier(MOD_ID, "stance_dugu_poison");
    public static final Identifier STANCE_ZHENMAI = new Identifier(MOD_ID, "stance_zhenmai");
    public static final Identifier STANCE_WOLIU = new Identifier(MOD_ID, "stance_woliu");
    public static final Identifier STANCE_TUIKE = new Identifier(MOD_ID, "stance_tuike");
    public static final Identifier LIMP_LEFT = new Identifier(MOD_ID, "limp_left");
    public static final Identifier LIMP_RIGHT = new Identifier(MOD_ID, "limp_right");
    public static final Identifier ARM_INJURED_LEFT = new Identifier(MOD_ID, "arm_injured_left");
    public static final Identifier ARM_INJURED_RIGHT = new Identifier(MOD_ID, "arm_injured_right");
    public static final Identifier EXHAUSTED_WALK = new Identifier(MOD_ID, "exhausted_walk");
    public static final Identifier DASH_FORWARD = new Identifier(MOD_ID, "dash_forward");
    public static final Identifier SLIDE_LOW = new Identifier(MOD_ID, "slide_low");
    public static final Identifier DOUBLE_JUMP = new Identifier(MOD_ID, "double_jump");

    public static final List<Identifier> IMPLEMENTATION_V1_ANIMATIONS = List.of(
        SWORD_SWING_RIGHT,
        MEDITATE_SIT,
        HURT_STAGGER,
        FIST_PUNCH_RIGHT,
        FIST_PUNCH_LEFT,
        PALM_STRIKE,
        SWORD_SLASH_DOWN,
        WINDUP_CHARGE,
        RELEASE_BURST,
        PARRY_BLOCK,
        DODGE_ROLL,
        HARVEST_CROUCH,
        LOOT_BEND,
        STEALTH_CROUCH,
        IDLE_BREATHE,
        NPC_PATROL_WALK,
        NPC_CHOP_TREE,
        NPC_MINE,
        NPC_CROUCH_WAVE,
        NPC_FLEE_RUN,
        FORGE_HAMMER,
        ALCHEMY_STIR,
        LINGTIAN_TILL,
        INVENTORY_REACH,
        STANCE_BAOMAI,
        STANCE_DUGU,
        STANCE_ZHENFA,
        STANCE_DUGU_POISON,
        STANCE_ZHENMAI,
        STANCE_WOLIU,
        STANCE_TUIKE,
        LIMP_LEFT,
        LIMP_RIGHT,
        ARM_INJURED_LEFT,
        ARM_INJURED_RIGHT,
        EXHAUSTED_WALK,
        BREAKTHROUGH_YINQI,
        BREAKTHROUGH_NINGMAI,
        BREAKTHROUGH_GUYUAN,
        BREAKTHROUGH_TONGLING,
        DEATH_COLLAPSE,
        DEATH_DISINTEGRATE,
        REBIRTH_WAKE
    );

    public static final List<Identifier> MOVEMENT_V1_ANIMATIONS = List.of(
        DASH_FORWARD,
        SLIDE_LOW,
        DOUBLE_JUMP
    );

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
