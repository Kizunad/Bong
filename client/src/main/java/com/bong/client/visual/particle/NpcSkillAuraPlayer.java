package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;

/**
 * NPC 三招专属粒子播放器 —— plan-skill-anim-fidelity-v1 §P5.1 ③。
 *
 * <p><b>为什么存在</b>：P5 之前 NPC 三招粒子全是借的——{@code heal_basic} 借医道
 * {@code bong:yidao_meridian_repair}、{@code buff_speed} 借真脉
 * {@code bong:jiemai_neutralize_dust}、{@code buff_defense} 借崩拳
 * {@code bong:burst_meridian_beng_quan}。后果有二：① NPC 施法与玩家同招粒子完全同形，
 * 旁观者分不清"对面 NPC 在补血"还是"有玩家在放医道"；② 借来的 id 命中玩家技能家族前缀，
 * 让 NPC 背景 cosmetic 误吃 Important 优先级、在拥挤 chunk 里挤掉玩家自己的技能反馈。
 *
 * <p>NPC 是 mob 实体、无 PlayerAnimator 通道（plan §8.1 #2），所以三招只有粒子 + 音效，
 * 形态按 plan 允许的"从简"处理；但 <b>id 与颜色强制独立</b>，构成绿 / 黄 / 蓝
 * 高分离色相三元组，保证远距离读招。逐招 spec 见 {@link Form}（与 plan §P5.1 ③ 表格逐格对拍，
 * 由 {@code SkillParticleSpecDocTest} 锁住）。
 */
public final class NpcSkillAuraPlayer implements VfxPlayer {
    public static final Identifier HEAL_BASIC = new Identifier("bong", "npc_heal_basic");
    public static final Identifier BUFF_SPEED = new Identifier("bong", "npc_buff_speed");
    public static final Identifier BUFF_DEFENSE = new Identifier("bong", "npc_buff_defense");

    /** {@link VfxBootstrap} 遍历注册用。 */
    public static final Identifier[] EVENT_IDS = { HEAL_BASIC, BUFF_SPEED, BUFF_DEFENSE };

    static final int HEAL_RGB = 0xA8E6CF;
    static final int SPEED_RGB = 0xE3C766;
    static final int DEFENSE_RGB = 0x5BA8C9;

    /** 与 server `emit_npc_skill_av` 的 `count: Some(12)` / `duration_ticks: Some(40)` 一致。 */
    static final int DEFAULT_COUNT = 12;
    static final int DEFAULT_LIFETIME_TICKS = 40;
    static final int MIN_COUNT = 1;
    static final int MAX_COUNT = 48;

    private static final double HEAL_BASE_HEIGHT = -0.5;
    private static final double HEAL_STEP_HEIGHT = 0.22;
    private static final int HEAL_STEP_COUNT = 4;
    private static final float HEAL_ALPHA = 0.85f;
    private static final float HEAL_SCALE = 0.55f;

    private static final double SPEED_HEIGHT = -0.6;
    private static final float SPEED_ALPHA = 0.75f;

    private static final float DEFENSE_ALPHA = 0.8f;
    private static final float DEFENSE_SCALE = 0.6f;

    /**
     * 逐招形态 —— <b>plan §P5.1 ③ spec 表的可执行副本</b>。
     *
     * <p>抽成枚举而非只在 {@link #play} 里比 id，是为了让 dispatch 与配色 / 运动 spec 都能在无
     * {@code MinecraftClient} 的单测里被断言，并与 plan 文档逐格对拍。
     */
    enum Form {
        /** 回血：绕身上升的绿色光点柱。 */
        HEAL_COLUMN(HEAL_BASIC, HEAL_RGB, Motion.COLUMN, 0.0, 0.4, 0.03),
        /** 加速：水平外冲的麦黄疾行尘。 */
        SPEED_DUST(BUFF_SPEED, SPEED_RGB, Motion.OUTRUSH, 0.22, 0.3, 0.01),
        /** 护体：贴身<b>真环绕</b>的青蓝护环。 */
        DEFENSE_RING(BUFF_DEFENSE, DEFENSE_RGB, Motion.ORBIT, 0.06, 0.6, 0.0);

        /**
         * 运动形态。{@link #ORBIT} 是 review r1 #2 的修复点——真绕圆心转（半径恒定），
         * 而非「只给切向初速度」的抛射（那会让护环直线散开、读成"炸开"而非"护住"）。
         */
        enum Motion { COLUMN, OUTRUSH, ORBIT }

        final Identifier eventId;
        final int fallbackRgb;
        final Motion motion;
        /** 水平速度（格/tick）。{@link Motion#ORBIT} 下是线速度。 */
        final double speed;
        /** 起始铺开半径（格）；{@link Motion#ORBIT} 下即环绕半径。 */
        final double radius;
        /** 垂直速度（格/tick）。 */
        final double verticalSpeed;

        Form(
            Identifier eventId,
            int fallbackRgb,
            Motion motion,
            double speed,
            double radius,
            double verticalSpeed
        ) {
            this.eventId = eventId;
            this.fallbackRgb = fallbackRgb;
            this.motion = motion;
            this.speed = speed;
            this.radius = radius;
            this.verticalSpeed = verticalSpeed;
        }
    }

    /** event_id → 形态。未登记 id 返回 {@code null}（调用方静默跳过，绝不抛）。 */
    static Form formFor(Identifier eventId) {
        if (eventId == null) {
            return null;
        }
        for (Form form : Form.values()) {
            if (form.eventId.equals(eventId)) {
                return form;
            }
        }
        return null;
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }
        SkillParticleSpawner.spawnAll(client, world, plan(payload));
    }

    /**
     * 纯函数发射规划：payload → 逐颗粒子描述符。不碰 {@code MinecraftClient}，供单测直接断言。
     *
     * @return 未登记 event_id 返回空列表
     */
    static List<SkillParticleSpawn> plan(VfxEventPayload.SpawnParticle payload) {
        Form form = formFor(payload.eventId());
        if (form == null) {
            return List.of();
        }

        double[] origin = payload.origin();
        int count = clampInt(payload.count().orElse(DEFAULT_COUNT), MIN_COUNT, MAX_COUNT);
        int rgb = payload.colorRgb().orElse(form.fallbackRgb);
        int lifetime = payload.durationTicks().orElse(DEFAULT_LIFETIME_TICKS);

        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            double cos = Math.cos(angle);
            double sin = Math.sin(angle);

            // switch 表达式（无 default）：新增 Motion 变体而忘了接分支 = 编译期撞红。
            SkillParticleSpawn spawn = switch (form.motion) {
                // 回血：绕身一圈、分四段身高错落的上升光点柱。
                case COLUMN -> new SkillParticleSpawn.Point(
                    origin[0] + cos * form.radius,
                    origin[1] + HEAL_BASE_HEIGHT + (i % HEAL_STEP_COUNT) * HEAL_STEP_HEIGHT,
                    origin[2] + sin * form.radius,
                    0.0, form.verticalSpeed, 0.0,
                    HEAL_SCALE, rgb, HEAL_ALPHA, lifetime,
                    SkillParticleSpawn.Sheet.QI_AURA
                );
                // 加速：脚边水平外冲的线粒子（与另两招的点粒子形态区分）。
                case OUTRUSH -> new SkillParticleSpawn.Pulse(
                    origin[0] + cos * form.radius,
                    origin[1] + SPEED_HEIGHT,
                    origin[2] + sin * form.radius,
                    cos * form.speed, form.verticalSpeed, sin * form.speed,
                    1.4, 0.5, 0.07,
                    rgb, SPEED_ALPHA, lifetime, SkillParticleSpawn.Sheet.QI_AURA
                );
                // 护体：贴身真环绕，圆心锚定 cast 瞬间 origin（同 burst 逆脉护体的取舍）。
                case ORBIT -> new SkillParticleSpawn.OrbitPoint(
                    new SkillParticleSpawn.OrbitSpec(
                        origin[0], origin[1], origin[2],
                        form.radius, angle, form.speed, form.verticalSpeed
                    ),
                    DEFENSE_SCALE, rgb, DEFENSE_ALPHA, lifetime,
                    SkillParticleSpawn.Sheet.LINGQI_RIPPLE
                );
            };
            spawns.add(spawn);
        }
        return List.copyOf(spawns);
    }

    private static int clampInt(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
