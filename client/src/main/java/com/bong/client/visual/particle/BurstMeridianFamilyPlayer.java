package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;

/**
 * 爆脉（burst_meridian）三招专属粒子播放器 —— plan-skill-anim-fidelity-v1 §P5.1 ②。
 *
 * <p><b>为什么存在</b>：P5 之前 {@code tie_shan_kao} / {@code xue_beng_bu} /
 * {@code ni_mai_hu_ti} 三招粒子全部借崩拳的 {@code bong:burst_meridian_beng_quan}
 * ——靠身撞击、突进步法、护体结印在旁观者眼里是同一团古铜色爆发。
 *
 * <p><b>设计取向与真脉相反</b>：真脉靠「同色系 + 明度阶梯」分化，爆脉则
 * <b>三招共用同一识别色 {@code #C58B3F}</b>（与崩拳本尊同色，统一爆脉家族识别），
 * 读招<b>完全靠形态</b>——撞击环贴地自旋 / 残影反向拖尾 / 逆流纹贴身环绕。
 *
 * <p>逐招 spec 见 {@link Form}（与 plan §P5.1 ② 表格逐格对拍，由
 * {@code SkillParticleSpecDocTest} 锁住）。崩拳本尊 {@code bong:burst_meridian_beng_quan}
 * 仍由 {@link BurstMeridianBengQuanPlayer} 承接，本类不接管。
 */
public final class BurstMeridianFamilyPlayer implements VfxPlayer {
    public static final Identifier TIE_SHAN_KAO =
        new Identifier("bong", "burst_meridian_tie_shan_kao");
    public static final Identifier XUE_BENG_BU =
        new Identifier("bong", "burst_meridian_xue_beng_bu");
    public static final Identifier NI_MAI_HU_TI =
        new Identifier("bong", "burst_meridian_ni_mai_hu_ti");

    /** {@link VfxBootstrap} 遍历注册用。 */
    public static final Identifier[] EVENT_IDS = { TIE_SHAN_KAO, XUE_BENG_BU, NI_MAI_HU_TI };

    /** 爆脉家族统一识别色（server `BURST_MERIDIAN_FAMILY_COLOR` 的 client 侧兜底）。 */
    static final int FAMILY_RGB = 0xC58B3F;

    static final int MAX_COUNT = 48;

    /** 撞击环贴地：server origin 抬到胸口高度，减 1 格落回脚下。 */
    private static final double IMPACT_GROUND_DROP = -1.0;
    private static final double IMPACT_Y_LIFT = 0.03;
    private static final float IMPACT_ALPHA = 0.72f;

    private static final double DASH_BACK_OFFSET = 0.15;
    private static final double DASH_LATERAL_SPREAD = 0.5;
    private static final double DASH_HEIGHT_BASE = -0.4;
    private static final double DASH_HEIGHT_SPAN = 1.1;
    private static final double DASH_RIBBON_TAIL = 0.02;
    private static final float DASH_ALPHA = 0.68f;

    /** 逆流纹双高度环：奇偶分上下两圈，读出「整个体表都在逆流」而不是一条腰带。 */
    private static final double COUNTERFLOW_LOW_RING = -0.25;
    private static final double COUNTERFLOW_HIGH_RING = 0.45;
    private static final float COUNTERFLOW_ALPHA = 0.8f;
    private static final float COUNTERFLOW_SCALE = 0.6f;

    /**
     * 逐招形态参数 —— <b>plan §P5.1 ② spec 表的可执行副本</b>。
     *
     * <p>抽成枚举而非只在 {@link #play} 里比 id，是为了让 dispatch 与 spec 都能在无
     * {@code MinecraftClient} 的单测里被断言，并与 plan 文档逐格对拍。
     *
     * <p>字段按形态取用，不适用者为 {@code 0.0}（例如残影没有半径、撞击环没有线速度）。
     */
    enum Form {
        /** 贴山靠：地面撞击冲击环（贴地静止 + 自旋，半径吃 strength）。 */
        IMPACT_RING(TIE_SHAN_KAO, 10, 4, 16, Motion.IMPACT, 0.0, 0.30, 0.45, 0.0, 0.05),
        /** 血崩步：步法残影短尾（沿 direction 反向拖出）。 */
        DASH_AFTERIMAGE(XUE_BENG_BU, 12, 2, 14, Motion.TRAIL, 0.25, 0.0, 0.0, 0.01, 0.0),
        /**
         * 逆脉护体：体表逆流纹（贴身<b>真环绕</b> + 缓慢上浮）。
         *
         * <p>lifetime 只有一个重发间隔，不是整个 60t 护体窗口——窗口由 server 逐环接力
         * 铺满，见 {@link #planBodyCounterflow}。
         */
        BODY_COUNTERFLOW(NI_MAI_HU_TI, 14, 4, 12, Motion.ORBIT, 0.08, 0.55, 0.0, 0.015, 0.0);

        /**
         * 运动形态。{@link #ORBIT} 真绕圆心转、半径由参数化路径恒定，不是「只给切向初速度」
         * 的抛射（那会让护体纹沿切线直线飞离身体）。
         */
        enum Motion { IMPACT, TRAIL, ORBIT }

        final Identifier eventId;
        final int defaultCount;
        /** count 下限：环形招 4（低于此读不出「环」），线性残影 2 即可成束。 */
        final int minCount;
        final int defaultLifetime;
        final Motion motion;
        /** 主运动速度（格/tick）。{@link Motion#ORBIT} 下是线速度。 */
        final double speed;
        /** 半径基值（格）。{@link Motion#IMPACT} 下与 {@link #radiusStrength} 合成。 */
        final double radiusBase;
        /** 半径的 strength 系数（格）：实际半径 = radiusBase + radiusStrength × strength。 */
        final double radiusStrength;
        /** 垂直速度（格/tick）。 */
        final double verticalSpeed;
        /** 贴地符圈自旋（rad/tick）。 */
        final double spinRate;

        Form(
            Identifier eventId,
            int defaultCount,
            int minCount,
            int defaultLifetime,
            Motion motion,
            double speed,
            double radiusBase,
            double radiusStrength,
            double verticalSpeed,
            double spinRate
        ) {
            this.eventId = eventId;
            this.defaultCount = defaultCount;
            this.minCount = minCount;
            this.defaultLifetime = defaultLifetime;
            this.motion = motion;
            this.speed = speed;
            this.radiusBase = radiusBase;
            this.radiusStrength = radiusStrength;
            this.verticalSpeed = verticalSpeed;
            this.spinRate = spinRate;
        }

        double radiusAt(double strength) {
            return radiusBase + radiusStrength * strength;
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
     * @return 未登记 event_id 返回空列表（接线错误不该让渲染线程抛异常）
     */
    static List<SkillParticleSpawn> plan(VfxEventPayload.SpawnParticle payload) {
        Form form = formFor(payload.eventId());
        if (form == null) {
            return List.of();
        }
        return switch (form.motion) {
            case IMPACT -> planImpactRing(form, payload);
            case TRAIL -> planDashAfterimage(form, payload);
            case ORBIT -> planBodyCounterflow(form, payload);
        };
    }

    /** 贴山靠：地面撞击冲击环。半径吃 strength，整圈自旋表达「碾压」。 */
    private static List<SkillParticleSpawn> planImpactRing(
        Form form,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = clampInt(payload.count().orElse(form.defaultCount), form.minCount, MAX_COUNT);
        int rgb = payload.colorRgb().orElse(FAMILY_RGB);
        double strength = clamp(payload.strength().orElse(0.95), 0.0, 1.0);
        int lifetime = payload.durationTicks().orElse(form.defaultLifetime);
        double halfSize = form.radiusAt(strength);

        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            spawns.add(new SkillParticleSpawn.Decal(
                origin[0] + Math.cos(angle) * halfSize,
                origin[1] + IMPACT_GROUND_DROP + IMPACT_Y_LIFT,
                origin[2] + Math.sin(angle) * halfSize,
                halfSize, IMPACT_Y_LIFT, angle, form.spinRate,
                rgb, IMPACT_ALPHA, lifetime, SkillParticleSpawn.Sheet.LINGQI_RIPPLE
            ));
        }
        return List.copyOf(spawns);
    }

    /**
     * 血崩步：步法残影短尾。
     *
     * <p>关键取向——ribbon 沿 {@code direction} 的<b>反向</b>拖出，残影落在身后，
     * 才读得出"人往前窜了"。顺向拖尾会读成"往前喷气"，语义相反。
     */
    private static List<SkillParticleSpawn> planDashAfterimage(
        Form form,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        double[] dir = ZhenmaiPulsePlayer.normalizedDirection(payload.direction().orElse(null));
        int count = clampInt(payload.count().orElse(form.defaultCount), form.minCount, MAX_COUNT);
        int rgb = payload.colorRgb().orElse(FAMILY_RGB);
        double strength = clamp(payload.strength().orElse(0.85), 0.0, 1.0);
        int lifetime = payload.durationTicks().orElse(form.defaultLifetime);

        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            // 沿身高分布 + 轻微横向抖动，构成一束残影而不是一条线。
            double t = count == 1 ? 0.0 : (double) i / (count - 1);
            double lateral = (t - 0.5) * DASH_LATERAL_SPREAD;
            spawns.add(new SkillParticleSpawn.Ribbon(
                origin[0] - dir[0] * DASH_BACK_OFFSET + lateral * -dir[2],
                origin[1] + DASH_HEIGHT_BASE + t * DASH_HEIGHT_SPAN,
                origin[2] - dir[2] * DASH_BACK_OFFSET + lateral * dir[0],
                -dir[0] * form.speed,
                form.verticalSpeed,
                -dir[2] * form.speed,
                0.14 * (0.6 + strength), DASH_RIBBON_TAIL,
                rgb, DASH_ALPHA, lifetime, SkillParticleSpawn.Sheet.QI_AURA
            ));
        }
        return List.copyOf(spawns);
    }

    /**
     * 逆脉护体：体表逆流纹。双高度环 + <b>真环绕</b>，贴身而非外扩（护体不是攻击）。
     *
     * <p>走 {@link SkillParticleSpawn.OrbitSpec}：位置由「圆心 + 恒定半径 × 角度」参数化给出，
     * 半径 0.55 格是构造期不变量，不会随 tick 外扩。
     *
     * <p><b>跟随施法者靠 server 接力</b>：{@code SpawnParticle} payload 只有世界坐标、没有实体
     * 标识，单个环无从锚定移动中的身体。故每个环只活一个重发间隔（12t），60t 护体窗口由 server
     * 的 {@code ni_mai_hu_ti_aura_vfx_tick} 以施法者**当前**位置逐环重发铺满——老环恰在新环生成
     * 的同 tick 消失，既贴身又不叠环。本类只负责把单个环画对，窗口节奏不在 client 侧。
     */
    private static List<SkillParticleSpawn> planBodyCounterflow(
        Form form,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = clampInt(payload.count().orElse(form.defaultCount), form.minCount, MAX_COUNT);
        int rgb = payload.colorRgb().orElse(FAMILY_RGB);
        double strength = clamp(payload.strength().orElse(0.7), 0.0, 1.0);
        int lifetime = payload.durationTicks().orElse(form.defaultLifetime);
        // 走 radiusAt 而非直接读 radiusBase：本形态的 radiusStrength 当前为 0，两者等价，
        // 但若将来 spec 表给这行写上 strength 系数，直读 radiusBase 会让渲染悄悄忽略它
        // ——而 spec 对拍测试只校验枚举字段，照样绿。半径口径只留一条路。
        double radius = form.radiusAt(strength);

        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            double heightOffset = (i % 2 == 0) ? COUNTERFLOW_LOW_RING : COUNTERFLOW_HIGH_RING;
            spawns.add(new SkillParticleSpawn.OrbitPoint(
                new SkillParticleSpawn.OrbitSpec(
                    origin[0], origin[1] + heightOffset, origin[2],
                    radius, angle, form.speed, form.verticalSpeed
                ),
                COUNTERFLOW_SCALE, rgb, COUNTERFLOW_ALPHA, lifetime,
                SkillParticleSpawn.Sheet.QI_AURA
            ));
        }
        return List.copyOf(spawns);
    }

    private static double clamp(double value, double lo, double hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static int clampInt(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
