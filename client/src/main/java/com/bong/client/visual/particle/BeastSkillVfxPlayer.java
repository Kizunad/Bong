package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;

/**
 * 兽类五招专属粒子播放器。
 *
 * <p>server 侧五个 {@code WildlifeSkill} 各发一个 event id；这里复用既有粒子描述符，
 * 用五种运动原语把动作读出来：扑击低位环绕尘粒、撕咬点状血屑、俯冲方向拖尾、践踏贴地
 * 冲击贴花、后踢定向脉冲。没有新增贴图，也不把五招压成同一个尘爆 player。
 *
 * <p>{@link #plan(VfxEventPayload.SpawnParticle)} 是不接触 Minecraft 运行时的纯规划函数，
 * 方便在 headless 单测中逐颗断言；真正把描述符交给 particle manager 的代码仍集中在
 * {@link SkillParticleSpawner}。
 */
public final class BeastSkillVfxPlayer implements VfxPlayer {
    public static final Identifier LION_POUNCE =
        new Identifier("bong", "fauna_lion_pounce");
    public static final Identifier LION_REND =
        new Identifier("bong", "fauna_lion_rend");
    public static final Identifier VULTURE_DIVE =
        new Identifier("bong", "fauna_vulture_dive");
    public static final Identifier HORSE_TRAMPLE =
        new Identifier("bong", "fauna_horse_trample");
    public static final Identifier HORSE_KICK =
        new Identifier("bong", "fauna_horse_kick");

    /** {@link VfxBootstrap} 遍历注册用；顺序与 server {@code WildlifeSkill} 保持一致。 */
    public static final Identifier[] EVENT_IDS = {
        LION_POUNCE, LION_REND, VULTURE_DIVE, HORSE_TRAMPLE, HORSE_KICK
    };

    private static final int MAX_COUNT = 48;

    /** 五种粒子原语各自承担一种动作语义，新增形态时 switch 会强制接线。 */
    enum Motion {
        LOW_ORBIT,
        BLOOD_SPRAY,
        AIR_TRAIL,
        GROUND_RING,
        KICK_PULSE
    }

    /** event id → 形态与服务端默认参数的可执行副本。 */
    enum Form {
        LION_POUNCE(BeastSkillVfxPlayer.LION_POUNCE,
            0xC3A57A, 12, 6, 18, 0.80, Motion.LOW_ORBIT),
        LION_REND(BeastSkillVfxPlayer.LION_REND,
            0xA83232, 10, 4, 16, 0.90, Motion.BLOOD_SPRAY),
        VULTURE_DIVE(BeastSkillVfxPlayer.VULTURE_DIVE,
            0xA8D8E8, 8, 3, 14, 0.75, Motion.AIR_TRAIL),
        HORSE_TRAMPLE(BeastSkillVfxPlayer.HORSE_TRAMPLE,
            0x8A6A44, 12, 6, 20, 0.95, Motion.GROUND_RING),
        HORSE_KICK(BeastSkillVfxPlayer.HORSE_KICK,
            0xE0B060, 8, 2, 12, 0.80, Motion.KICK_PULSE);

        final Identifier eventId;
        final int fallbackRgb;
        final int defaultCount;
        final int minCount;
        final int defaultLifetime;
        final double defaultStrength;
        final Motion motion;

        Form(
            Identifier eventId,
            int fallbackRgb,
            int defaultCount,
            int minCount,
            int defaultLifetime,
            double defaultStrength,
            Motion motion
        ) {
            this.eventId = eventId;
            this.fallbackRgb = fallbackRgb;
            this.defaultCount = defaultCount;
            this.minCount = minCount;
            this.defaultLifetime = defaultLifetime;
            this.defaultStrength = defaultStrength;
            this.motion = motion;
        }
    }

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
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        SkillParticleSpawner.spawnAll(client, world, plan(payload));
    }

    /**
     * 纯函数发射规划。未登记 event id 返回空列表，避免错误路由在渲染线程抛异常。
     */
    static List<SkillParticleSpawn> plan(VfxEventPayload.SpawnParticle payload) {
        if (payload == null) {
            return List.of();
        }
        Form form = formFor(payload.eventId());
        if (form == null) {
            return List.of();
        }

        int count = Math.max(
            form.minCount,
            Math.min(MAX_COUNT, payload.count().orElse(form.defaultCount))
        );
        int rgb = payload.colorRgb().orElse(form.fallbackRgb);
        double strength = GameplayVfxUtil.strength(payload, form.defaultStrength);
        int lifetime = GameplayVfxUtil.duration(payload, form.defaultLifetime);

        return switch (form.motion) {
            case LOW_ORBIT -> planPounce(payload, count, rgb, strength, lifetime);
            case BLOOD_SPRAY -> planRend(payload, count, rgb, strength, lifetime);
            case AIR_TRAIL -> planDive(payload, count, rgb, strength, lifetime);
            case GROUND_RING -> planTrample(payload, count, rgb, strength, lifetime);
            case KICK_PULSE -> planKick(payload, count, rgb, strength, lifetime);
        };
    }

    /** 扑击：脚边尘粒不是静态圆环，而是低位真环绕并带轻微上扬。 */
    private static List<SkillParticleSpawn> planPounce(
        VfxEventPayload.SpawnParticle payload,
        int count,
        int rgb,
        double strength,
        int lifetime
    ) {
        double[] origin = payload.origin();
        double radius = 0.34 + strength * 0.18;
        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            double centerY = origin[1] - 0.55 + (i % 3) * 0.04;
            spawns.add(new SkillParticleSpawn.OrbitPoint(
                new SkillParticleSpawn.OrbitSpec(
                    origin[0], centerY, origin[2],
                    radius, angle, 0.075 + strength * 0.025, 0.035 + strength * 0.02
                ),
                0.34f, rgb, 0.72f, lifetime, SkillParticleSpawn.Sheet.QI_AURA
            ));
        }
        return List.copyOf(spawns);
    }

    /** 撕咬：红色点状飞屑从目标方向附近炸开，保持与线性招式不同的点粒子轮廓。 */
    private static List<SkillParticleSpawn> planRend(
        VfxEventPayload.SpawnParticle payload,
        int count,
        int rgb,
        double strength,
        int lifetime
    ) {
        double[] origin = payload.origin();
        double[] direction = ZhenmaiPulsePlayer.normalizedDirection(payload.direction().orElse(null));
        double[] side = ZhenmaiPulsePlayer.perpendicular(direction);
        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double spread = count == 1 ? 0.0 : ((double) i / (count - 1)) * 2.0 - 1.0;
            double lateral = spread * 0.22;
            double speed = 0.055 + strength * 0.035;
            spawns.add(new SkillParticleSpawn.Point(
                origin[0] + direction[0] * 0.30 + side[0] * lateral,
                origin[1] + 0.65 + ((i % 3) - 1) * 0.08,
                origin[2] + direction[2] * 0.30 + side[2] * lateral,
                direction[0] * speed + side[0] * spread * 0.025,
                0.035 + (i % 3) * 0.015,
                direction[2] * speed + side[2] * spread * 0.025,
                0.12f, rgb, 0.82f, lifetime, SkillParticleSpawn.Sheet.LINGQI_RIPPLE
            ));
        }
        return List.copyOf(spawns);
    }

    /** 俯冲：Ribbon 沿施法方向留下气流拖尾，头宽尾窄，不是径向尘爆。 */
    private static List<SkillParticleSpawn> planDive(
        VfxEventPayload.SpawnParticle payload,
        int count,
        int rgb,
        double strength,
        int lifetime
    ) {
        double[] origin = payload.origin();
        double[] direction = ZhenmaiPulsePlayer.normalizedDirection(payload.direction().orElse(null));
        double[] side = ZhenmaiPulsePlayer.perpendicular(direction);
        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double t = count == 1 ? 0.0 : (double) i / (count - 1);
            double lateral = (t - 0.5) * 0.50;
            double back = 0.30 + t * 0.18;
            double speed = 0.10 + strength * 0.05;
            spawns.add(new SkillParticleSpawn.Ribbon(
                origin[0] - direction[0] * back + side[0] * lateral,
                origin[1] + 0.45 + (t - 0.5) * 0.22,
                origin[2] - direction[2] * back + side[2] * lateral,
                direction[0] * speed, direction[1] * speed, direction[2] * speed,
                0.16 + strength * 0.05, 0.025,
                rgb, 0.68f, lifetime, SkillParticleSpawn.Sheet.QI_AURA
            ));
        }
        return List.copyOf(spawns);
    }

    /** 践踏：低处铺开一圈 Decal 冲击片，静止贴地而不是向外发射。 */
    private static List<SkillParticleSpawn> planTrample(
        VfxEventPayload.SpawnParticle payload,
        int count,
        int rgb,
        double strength,
        int lifetime
    ) {
        double[] origin = payload.origin();
        double radius = 0.42 + strength * 0.34;
        double halfSize = 0.18 + strength * 0.05;
        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            spawns.add(new SkillParticleSpawn.Decal(
                origin[0] + Math.cos(angle) * radius,
                origin[1] + 0.03,
                origin[2] + Math.sin(angle) * radius,
                halfSize, 0.03, angle, 0.08 + strength * 0.04,
                rgb, 0.74f, lifetime, SkillParticleSpawn.Sheet.LINGQI_RIPPLE
            ));
        }
        return List.copyOf(spawns);
    }

    /** 后踢：短 Pulse 沿脚的方向打出扇面，和俯冲的 Ribbon 明确区分。 */
    private static List<SkillParticleSpawn> planKick(
        VfxEventPayload.SpawnParticle payload,
        int count,
        int rgb,
        double strength,
        int lifetime
    ) {
        double[] origin = payload.origin();
        double[] direction = ZhenmaiPulsePlayer.normalizedDirection(payload.direction().orElse(null));
        double[] side = ZhenmaiPulsePlayer.perpendicular(direction);
        List<SkillParticleSpawn> spawns = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double spread = count == 1 ? 0.0 : ((double) i / (count - 1)) * 2.0 - 1.0;
            double lateral = spread * 0.35;
            double speed = 0.12 + strength * 0.06;
            spawns.add(new SkillParticleSpawn.Pulse(
                origin[0] + direction[0] * 0.38 + side[0] * lateral,
                origin[1] + 0.35 + (i % 2) * 0.10,
                origin[2] + direction[2] * 0.38 + side[2] * lateral,
                direction[0] * speed + side[0] * spread * 0.035,
                0.04 + strength * 0.025,
                direction[2] * speed + side[2] * spread * 0.035,
                1.30, 0.45, 0.07,
                rgb, 0.76f, lifetime, SkillParticleSpawn.Sheet.QI_AURA
            ));
        }
        return List.copyOf(spawns);
    }
}
