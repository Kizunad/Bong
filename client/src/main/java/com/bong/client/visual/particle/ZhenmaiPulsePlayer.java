package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

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
 * <p>逐招 spec（数量列为 payload 缺省时的兜底；server 常态会传 {@code count}）：
 * <table border="1">
 *   <caption>真脉 5 招粒子 spec</caption>
 *   <tr><th>event_id</th><th>脉冲数</th><th>穴位点</th><th>lifetime</th>
 *       <th>速度/方向</th><th>颜色</th><th>spawn</th></tr>
 *   <tr><td>zhenmai_parry_flash</td><td>8</td><td>3</td><td>20t</td>
 *       <td>沿 direction 前向 0.35/t</td><td>#D4AF6A</td><td>burst</td></tr>
 *   <tr><td>zhenmai_neutralize_dust</td><td>10</td><td>4</td><td>20t</td>
 *       <td>径向外散 0.12/t + 下沉 −0.02/t</td><td>#C9A05C</td><td>radial</td></tr>
 *   <tr><td>zhenmai_multipoint_ring</td><td>16</td><td>8</td><td>20t</td>
 *       <td>切向环绕 0.10/t</td><td>#E0C27E</td><td>radial</td></tr>
 *   <tr><td>zhenmai_harden_shell</td><td>8</td><td>6</td><td>20t</td>
 *       <td>向心内收 −0.06/t + 上浮 0.01/t</td><td>#B8944F</td><td>radial 双层</td></tr>
 *   <tr><td>zhenmai_sever_snap</td><td>18</td><td>2</td><td>20t</td>
 *       <td>沿 direction 爆冲 0.55/t</td><td>#F2D68A</td><td>burst</td></tr>
 * </table>
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

    /** 默认 lifetime，与 server `emit_skill_feedback` 的 duration_ticks 一致。 */
    private static final int DEFAULT_LIFETIME_TICKS = 20;

    /**
     * 逐招形态参数。抽成枚举而非散落 if-else，是为了让「每招的 spec」可被单测直接读取断言
     * ——形态参数漂移（比如有人把两招的运动方式改成一样）会立刻撞红。
     */
    enum Form {
        /** 极限弹反：沿来袭方向的一记前向金闪，穴位点少而亮。 */
        PARRY(PARRY_FLASH, 0xD4AF6A, 8, 3, Motion.FORWARD, 0.35, 0.55),
        /** 卸力中和：贴地径向散开并下沉的金尘。 */
        NEUTRALIZE(NEUTRALIZE_DUST, 0xC9A05C, 10, 4, Motion.RADIAL_OUT, 0.12, 0.40),
        /** 多点连环：腰高切向环绕，穴位点等角铺满一圈。 */
        MULTIPOINT(MULTIPOINT_RING, 0xE0C27E, 16, 8, Motion.TANGENTIAL, 0.10, 0.50),
        /** 硬化护脉：向心内收的双层护壳，最沉的暗金。 */
        HARDEN(HARDEN_SHELL, 0xB8944F, 8, 6, Motion.RADIAL_IN, 0.06, 0.45),
        /** 断脉：沿指向爆冲的最亮金闪，穴位点仅 2 个（爆点而非环）。 */
        SEVER(SEVER_SNAP, 0xF2D68A, 18, 2, Motion.FORWARD, 0.55, 0.70);

        /** 运动形态。分类而非裸速度，便于测试断言「两招形态确实不同」。 */
        enum Motion { FORWARD, RADIAL_OUT, RADIAL_IN, TANGENTIAL }

        final Identifier eventId;
        final int fallbackRgb;
        final int defaultPulses;
        final int acupoints;
        final Motion motion;
        /** 主运动速度（格/tick）。 */
        final double speed;
        /** 脉冲起始铺开半径（格）。 */
        final double radius;

        Form(
            Identifier eventId,
            int fallbackRgb,
            int defaultPulses,
            int acupoints,
            Motion motion,
            double speed,
            double radius
        ) {
            this.eventId = eventId;
            this.fallbackRgb = fallbackRgb;
            this.defaultPulses = defaultPulses;
            this.acupoints = acupoints;
            this.motion = motion;
            this.speed = speed;
            this.radius = radius;
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
        Form form = formFor(payload.eventId());
        if (form == null) {
            // 注册表把非真脉 id 派到本 player 属接线错误；静默跳过而不是画错东西。
            return;
        }

        double[] origin = payload.origin();
        double[] dir = normalizedDirection(payload.direction().orElse(null));
        float[] color = rgb(payload.colorRgb().orElse(form.fallbackRgb));
        double strength = clamp(payload.strength().orElse(0.75), 0.0, 1.0);
        int lifetime = payload.durationTicks().orElse(DEFAULT_LIFETIME_TICKS);
        int pulses = clampInt(payload.count().orElse(form.defaultPulses), 1, 48);

        emitPulses(client, world, form, origin, dir, color, strength, lifetime, pulses);
        emitAcupoints(client, world, form, origin, color, lifetime);
    }

    /** 第一层：沿经脉走向的 {@link BongLineParticle} 短脉冲。 */
    private static void emitPulses(
        MinecraftClient client,
        ClientWorld world,
        Form form,
        double[] origin,
        double[] dir,
        float[] color,
        double strength,
        int lifetime,
        int pulses
    ) {
        double[] side = perpendicular(dir);
        float alpha = (float) clamp(0.45 + 0.5 * strength, 0.1, 1.0);

        for (int i = 0; i < pulses; i++) {
            double angle = Math.PI * 2.0 * i / pulses;
            double cos = Math.cos(angle);
            double sin = Math.sin(angle);

            double px;
            double py;
            double pz;
            double vx;
            double vy;
            double vz;

            switch (form.motion) {
                case FORWARD -> {
                    // 沿 direction 前冲，横向按 side 轴铺成一道扇面。
                    double spread = pulses == 1 ? 0.0 : ((double) i / (pulses - 1)) * 2.0 - 1.0;
                    double lateral = spread * form.radius;
                    px = origin[0] + side[0] * lateral;
                    py = origin[1] + side[1] * lateral;
                    pz = origin[2] + side[2] * lateral;
                    vx = dir[0] * form.speed;
                    vy = dir[1] * form.speed;
                    vz = dir[2] * form.speed;
                }
                case RADIAL_OUT -> {
                    px = origin[0] + cos * form.radius;
                    py = origin[1];
                    pz = origin[2] + sin * form.radius;
                    vx = cos * form.speed;
                    vy = -0.02;
                    vz = sin * form.speed;
                }
                case RADIAL_IN -> {
                    // 双层护壳：奇偶粒子分内外两圈，向心收拢。
                    double layer = (i % 2 == 0) ? 1.0 : 1.5;
                    px = origin[0] + cos * form.radius * layer;
                    py = origin[1] + (i % 2 == 0 ? 0.0 : 0.35);
                    pz = origin[2] + sin * form.radius * layer;
                    vx = -cos * form.speed;
                    vy = 0.01;
                    vz = -sin * form.speed;
                }
                case TANGENTIAL -> {
                    px = origin[0] + cos * form.radius;
                    py = origin[1];
                    pz = origin[2] + sin * form.radius;
                    // 切向 = 半径向量绕 Y 轴转 90°。
                    vx = -sin * form.speed;
                    vy = 0.0;
                    vz = cos * form.speed;
                }
                default -> {
                    return;
                }
            }

            BongLineParticle particle = new BongLineParticle(world, px, py, pz, vx, vy, vz);
            particle.setLineShape(1.6, 0.7, 0.08 + 0.10 * strength);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(alpha);
            particle.setMaxAgePublic(lifetime);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /**
     * 第二层：驻留的穴位点。数量固定由形态自持（不吃 payload.count——那个参数是给脉冲用的），
     * 等角铺在腰高一圈，几乎不动，用来读「这一招点了几个穴」。
     */
    private static void emitAcupoints(
        MinecraftClient client,
        ClientWorld world,
        Form form,
        double[] origin,
        float[] color,
        int lifetime
    ) {
        for (int i = 0; i < form.acupoints; i++) {
            double angle = Math.PI * 2.0 * i / form.acupoints;
            BongSpriteParticle particle = new BongSpriteParticle(
                world,
                origin[0] + Math.cos(angle) * form.radius * 0.6,
                origin[1] + 0.15,
                origin[2] + Math.sin(angle) * form.radius * 0.6,
                0.0, 0.008, 0.0
            );
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.9f);
            particle.setScalePublic(0.7f);
            // 穴位点比脉冲活得久一点，让"点了哪些穴"在脉冲散后仍可读。
            particle.setMaxAgePublic(lifetime + 6);
            if (BongParticles.lingqiRippleSprites != null) {
                particle.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
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

    private static float[] rgb(int rgb) {
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f,
        };
    }

    private static double clamp(double value, double lo, double hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static int clampInt(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
