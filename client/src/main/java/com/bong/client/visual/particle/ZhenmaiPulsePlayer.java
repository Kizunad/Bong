package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;

/**
 * 真脉（zhenmai）5 招专属粒子播放器 —— plan-skill-anim-fidelity-v1 §P5.1 ①。
 *
 * <p><b>为什么存在</b>：P5 之前 5 招挤在 3 个 {@code bong:jiemai_*} event_id 上
 * （multipoint 借 parry、harden 借 neutralize），且三者在 {@code VfxBootstrap} 里
 * <b>全部注册到剑气 {@link SwordQiSlashPlayer}</b>——真脉招式在旁观者眼里全是剑气斩弧，
 * 五招互不可辨，违背「招式 A/V 差异化」红线。
 *
 * <p><b>形态</b>：统一由「{@link BongLineParticle} 沿经脉走向的短脉冲 +
 * {@link BongSpriteParticle} 驻留穴位点」两层构成，逐招靠运动形态与金脉明度阶梯分化。
 * 金脉色系 anchor {@code #D4AF6A}，明度序与招式烈度同序：
 * {@code harden(#B8944F) < neutralize(#C9A05C) < parry(#D4AF6A) < multipoint(#E0C27E)
 * < sever(#F2D68A)}。
 *
 * <p><b>贴图零新增</b>：短脉冲复用 {@code qi_aura}，穴位点复用 {@code lingqi_ripple}。
 *
 * <p><b>逐招 spec 是 {@link Form} 枚举，不是本注释</b>——plan §P5.1 ① 的表格与 {@code Form}
 * 各字段逐格对应，由 {@code SkillParticleSpecDocTest} 直接解析 plan markdown 对拍；
 * 任一侧改动而另一侧没跟，测试撞红。所以这里不再重抄一份数值表（review r1：抄第三份必然漂）。
 */
public final class ZhenmaiPulsePlayer implements VfxPlayer {
    public static final Identifier PARRY_FLASH = new Identifier("bong", "zhenmai_parry_flash");
    public static final Identifier NEUTRALIZE_DUST =
        new Identifier("bong", "zhenmai_neutralize_dust");
    public static final Identifier MULTIPOINT_RING =
        new Identifier("bong", "zhenmai_multipoint_ring");
    public static final Identifier HARDEN_SHELL = new Identifier("bong", "zhenmai_harden_shell");
    public static final Identifier SEVER_SNAP = new Identifier("bong", "zhenmai_sever_snap");

    /** {@link VfxBootstrap} 遍历注册用——新增招式只改这里一处。 */
    public static final Identifier[] EVENT_IDS = {
        PARRY_FLASH, NEUTRALIZE_DUST, MULTIPOINT_RING, HARDEN_SHELL, SEVER_SNAP
    };

    /** 默认 lifetime，与 server `emit_skill_feedback` 的 `duration_ticks: Some(20)` 一致。 */
    static final int DEFAULT_LIFETIME_TICKS = 20;

    /** 脉冲数上限——server `count` 异常放大时挡住粒子风暴。 */
    static final int MAX_PULSES = 48;

    private static final double PULSE_LENGTH_FACTOR = 1.6;
    private static final double PULSE_MIN_LENGTH = 0.7;
    private static final float ACUPOINT_ALPHA = 0.9f;
    private static final float ACUPOINT_SCALE = 0.7f;
    /** 穴位点铺在脉冲环内侧，读作「点在身上」而非「炸在外圈」。 */
    private static final double ACUPOINT_RADIUS_RATIO = 0.6;
    private static final double ACUPOINT_HEIGHT = 0.15;

    /**
     * 逐招形态参数 —— <b>plan §P5.1 ① spec 表的可执行副本</b>。
     *
     * <p>抽成枚举而非散落 if-else，是为了让「每招的 spec」既能被单测直接读取断言、又能与 plan
     * 文档逐格对拍；形态参数漂移（比如有人把两招的运动方式改成一样）会立刻撞红。
     */
    enum Form {
        /** 极限弹反：沿来袭方向的一记前向金闪，穴位点少而亮、且静止。 */
        PARRY(PARRY_FLASH, 0xD4AF6A, 8, 3, Motion.FORWARD, 0.35, 0.55, 0.0, 0.0),
        /** 卸力中和：贴地径向散开并下沉的金尘，穴位点随之微沉。 */
        NEUTRALIZE(NEUTRALIZE_DUST, 0xC9A05C, 10, 4, Motion.RADIAL_OUT, 0.12, 0.40, -0.02, -0.02),
        /** 多点连环：腰高<b>真环绕</b>，穴位点等角铺满一圈且驻留可数。 */
        MULTIPOINT(MULTIPOINT_RING, 0xE0C27E, 16, 8, Motion.ORBIT, 0.10, 0.50, 0.0, 0.0),
        /** 硬化护脉：向心内收的双层护壳，最沉的暗金，穴位点随壳微浮。 */
        HARDEN(HARDEN_SHELL, 0xB8944F, 8, 6, Motion.RADIAL_IN, 0.06, 0.45, 0.01, 0.01),
        /** 断脉：沿指向爆冲的最亮金闪，穴位点仅 2 个（爆点而非环）且钉住不动。 */
        SEVER(SEVER_SNAP, 0xF2D68A, 18, 2, Motion.FORWARD, 0.55, 0.70, 0.0, 0.0);

        /**
         * 运动形态。分类而非裸速度，便于测试断言「两招形态确实不同」。
         *
         * <p>{@link #ORBIT} 是 review r1 #2 的修复点：它走 {@link SkillParticleSpawn.OrbitPulse}
         * 真绕圆心转，与「只给切向初速度」的抛射有本质区别（后者会直线飞离）。
         */
        enum Motion { FORWARD, RADIAL_OUT, RADIAL_IN, ORBIT }

        final Identifier eventId;
        final int fallbackRgb;
        final int defaultPulses;
        final int acupoints;
        final Motion motion;
        /** 主运动速度（格/tick）。{@link Motion#ORBIT} 下是<b>线</b>速度，角速度 = 它 / 半径。 */
        final double speed;
        /** 脉冲起始铺开半径（格）；{@link Motion#ORBIT} 下即环绕半径。 */
        final double radius;
        /** 脉冲垂直速度（格/tick）：负 = 下沉，正 = 上浮。 */
        final double pulseVertical;
        /**
         * 穴位点垂直漂移（格/tick）。
         *
         * <p>review r1 #1：此前<b>所有招</b>统一硬编 {@code 0.008} 上浮且 lifetime 擅自 {@code +6}，
         * 而 plan spec 写的是「parry 穴位点静止 / lifetime 20t」——实现静默偏离自家 spec。
         * 现改为逐招显式配置且逐格写进 plan 表；穴位点 lifetime 与脉冲一致，不再私自延长。
         */
        final double acupointDrift;

        Form(
            Identifier eventId,
            int fallbackRgb,
            int defaultPulses,
            int acupoints,
            Motion motion,
            double speed,
            double radius,
            double pulseVertical,
            double acupointDrift
        ) {
            this.eventId = eventId;
            this.fallbackRgb = fallbackRgb;
            this.defaultPulses = defaultPulses;
            this.acupoints = acupoints;
            this.motion = motion;
            this.speed = speed;
            this.radius = radius;
            this.pulseVertical = pulseVertical;
            this.acupointDrift = acupointDrift;
        }
    }

    /** event_id → 形态。未登记 id 返回 {@code null}（调用方据此静默跳过，绝不抛）。 */
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
     * @return 未登记 event_id 返回空列表（注册表把非真脉 id 派进来属接线错误，静默跳过而不是画错东西）
     */
    static List<SkillParticleSpawn> plan(VfxEventPayload.SpawnParticle payload) {
        Form form = formFor(payload.eventId());
        if (form == null) {
            return List.of();
        }

        double[] origin = payload.origin();
        double[] dir = normalizedDirection(payload.direction().orElse(null));
        int rgb = payload.colorRgb().orElse(form.fallbackRgb);
        double strength = clamp(payload.strength().orElse(0.75), 0.0, 1.0);
        int lifetime = payload.durationTicks().orElse(DEFAULT_LIFETIME_TICKS);
        int pulses = clampInt(payload.count().orElse(form.defaultPulses), 1, MAX_PULSES);

        List<SkillParticleSpawn> spawns = new ArrayList<>(pulses + form.acupoints);
        addPulses(spawns, form, origin, dir, rgb, strength, lifetime, pulses);
        addAcupoints(spawns, form, origin, rgb, lifetime);
        return List.copyOf(spawns);
    }

    /** 第一层：沿经脉走向的短脉冲。 */
    private static void addPulses(
        List<SkillParticleSpawn> out,
        Form form,
        double[] origin,
        double[] dir,
        int rgb,
        double strength,
        int lifetime,
        int pulses
    ) {
        double[] side = perpendicular(dir);
        float alpha = (float) clamp(0.45 + 0.5 * strength, 0.1, 1.0);
        double halfWidth = 0.08 + 0.10 * strength;

        for (int i = 0; i < pulses; i++) {
            double angle = Math.PI * 2.0 * i / pulses;
            double cos = Math.cos(angle);
            double sin = Math.sin(angle);

            // switch 表达式（无 default）：新增 Motion 变体而忘了接分支 = 编译期撞红。
            // 不用「早退 + 带 default 的 switch 语句」——那样漏接分支会静默不发脉冲，
            // 而穴位点照发，连「该招没粒子」的非空断言都兜不住（与另两个 player 统一）。
            SkillParticleSpawn spawn = switch (form.motion) {
                // 真环绕：交给 OrbitSpec 持圆心 + 逐 tick 转角，半径恒定（review r1 #2）。
                case ORBIT -> new SkillParticleSpawn.OrbitPulse(
                    new SkillParticleSpawn.OrbitSpec(
                        origin[0], origin[1], origin[2],
                        form.radius, angle, form.speed, form.pulseVertical
                    ),
                    PULSE_LENGTH_FACTOR, PULSE_MIN_LENGTH, halfWidth,
                    rgb, alpha, lifetime, SkillParticleSpawn.Sheet.QI_AURA
                );
                // 沿 direction 前冲，横向按 side 轴铺成一道扇面。
                // 竖直分量来自 direction 本身（可俯可仰），不取 spec 的固定垂直速度。
                case FORWARD -> {
                    double spread = pulses == 1 ? 0.0 : ((double) i / (pulses - 1)) * 2.0 - 1.0;
                    double lateral = spread * form.radius;
                    yield pulse(
                        origin[0] + side[0] * lateral,
                        origin[1] + side[1] * lateral,
                        origin[2] + side[2] * lateral,
                        dir[0] * form.speed, dir[1] * form.speed, dir[2] * form.speed,
                        halfWidth, rgb, alpha, lifetime);
                }
                case RADIAL_OUT -> pulse(
                    origin[0] + cos * form.radius,
                    origin[1],
                    origin[2] + sin * form.radius,
                    cos * form.speed, form.pulseVertical, sin * form.speed,
                    halfWidth, rgb, alpha, lifetime);
                // 双层护壳：奇偶粒子分内外两圈，向心收拢。
                case RADIAL_IN -> {
                    double layer = (i % 2 == 0) ? 1.0 : 1.5;
                    yield pulse(
                        origin[0] + cos * form.radius * layer,
                        origin[1] + (i % 2 == 0 ? 0.0 : 0.35),
                        origin[2] + sin * form.radius * layer,
                        -cos * form.speed, form.pulseVertical, -sin * form.speed,
                        halfWidth, rgb, alpha, lifetime);
                }
            };
            out.add(spawn);
        }
    }

    /** 三条直线形态共用的 Pulse 构造（几何各自算好，材质/贴图参数在此收口）。 */
    private static SkillParticleSpawn.Pulse pulse(
        double px, double py, double pz,
        double vx, double vy, double vz,
        double halfWidth, int rgb, float alpha, int lifetime
    ) {
        return new SkillParticleSpawn.Pulse(
            px, py, pz, vx, vy, vz,
            PULSE_LENGTH_FACTOR, PULSE_MIN_LENGTH, halfWidth,
            rgb, alpha, lifetime, SkillParticleSpawn.Sheet.QI_AURA
        );
    }

    /**
     * 第二层：驻留的穴位点。数量固定由形态自持（不吃 payload.count——那个参数是给脉冲用的），
     * 等角铺在腰高一圈，用来读「这一招点了几个穴」。
     *
     * <p>lifetime 与脉冲<b>严格一致</b>：review r1 #1 之前这里私自 {@code +6} tick，与 plan spec
     * 表的「20t」不符——spec 是契约，要延长得先改 spec。
     */
    private static void addAcupoints(
        List<SkillParticleSpawn> out,
        Form form,
        double[] origin,
        int rgb,
        int lifetime
    ) {
        for (int i = 0; i < form.acupoints; i++) {
            double angle = Math.PI * 2.0 * i / form.acupoints;
            out.add(new SkillParticleSpawn.Point(
                origin[0] + Math.cos(angle) * form.radius * ACUPOINT_RADIUS_RATIO,
                origin[1] + ACUPOINT_HEIGHT,
                origin[2] + Math.sin(angle) * form.radius * ACUPOINT_RADIUS_RATIO,
                0.0, form.acupointDrift, 0.0,
                ACUPOINT_SCALE, rgb, ACUPOINT_ALPHA, lifetime,
                SkillParticleSpawn.Sheet.LINGQI_RIPPLE
            ));
        }
    }

    /** 归一化方向；缺省 / 退化向量回退到 +X（与 server 端 direction 校验同语义）。 */
    static double[] normalizedDirection(double[] dir) {
        if (dir == null || dir.length != 3) {
            return new double[] { 1.0, 0.0, 0.0 };
        }
        double length = Math.sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
        if (!(length > 1e-6) || !Double.isFinite(length)) {
            return new double[] { 1.0, 0.0, 0.0 };
        }
        return new double[] { dir[0] / length, dir[1] / length, dir[2] / length };
    }

    /** 取一条与 {@code dir} 垂直的水平轴，用于扇面铺开。dir 竖直时回退到 +Z。 */
    static double[] perpendicular(double[] dir) {
        double[] side = {
            dir[1] * 0.0 - dir[2] * 1.0,
            dir[2] * 0.0 - dir[0] * 0.0,
            dir[0] * 1.0 - dir[1] * 0.0,
        };
        double length = Math.sqrt(side[0] * side[0] + side[1] * side[1] + side[2] * side[2]);
        if (!(length > 1e-6)) {
            return new double[] { 0.0, 0.0, 1.0 };
        }
        return new double[] { side[0] / length, side[1] / length, side[2] / length };
    }

    private static double clamp(double value, double lo, double hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static int clampInt(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
