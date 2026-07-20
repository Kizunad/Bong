package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * 爆脉（burst_meridian）三招专属粒子播放器 —— plan-skill-anim-fidelity-v1 §P5.1 ②。
 *
 * <p><b>为什么存在</b>：P5 之前 {@code tie_shan_kao} / {@code xue_beng_bu} /
 * {@code ni_mai_hu_ti} 三招粒子全部借崩拳的 {@code bong:burst_meridian_beng_quan}
 * ——靠身撞击、突进步法、护体结印在旁观者眼里是同一团古铜色爆发。
 *
 * <p><b>设计取向与真脉相反</b>：真脉靠「同色系 + 明度阶梯」分化，爆脉则
 * <b>三招共用同一识别色 {@code #C58B3F}</b>（与崩拳本尊同色，统一爆脉家族识别），
 * 读招<b>完全靠形态</b>：
 * <table border="1">
 *   <caption>爆脉 3 招粒子 spec</caption>
 *   <tr><th>event_id</th><th>基类</th><th>数量</th><th>lifetime</th>
 *       <th>速度/方向</th><th>spawn</th><th>贴图</th></tr>
 *   <tr><td>burst_meridian_tie_shan_kao</td><td>{@link BongGroundDecalParticle}</td>
 *       <td>10</td><td>cast_ticks</td><td>贴地静止 + 自旋 0.05 rad/t</td>
 *       <td>radial 地面撞击冲击环</td><td>lingqi_ripple</td></tr>
 *   <tr><td>burst_meridian_xue_beng_bu</td><td>{@link BongRibbonParticle}</td>
 *       <td>12</td><td>cast_ticks</td><td>沿 direction <b>反向</b>拖尾 0.25/t</td>
 *       <td>continuous 步法短尾</td><td>qi_aura</td></tr>
 *   <tr><td>burst_meridian_ni_mai_hu_ti</td><td>{@link BongSpriteParticle}</td>
 *       <td>14</td><td>60t</td><td>切向环绕 0.08/t + 上浮 0.015/t</td>
 *       <td>radial 双高度体表纹</td><td>qi_aura</td></tr>
 * </table>
 *
 * <p>崩拳本尊 {@code bong:burst_meridian_beng_quan} 仍由
 * {@link BurstMeridianBengQuanPlayer} 承接，本类不接管。
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

    /**
     * 逐招形态参数。抽成枚举而非只在 {@link #play} 里比 id，是为了让 dispatch 与 spec
     * 都能在无 MinecraftClient 的单测里被断言（{@link #formFor}）。
     */
    enum Form {
        /** 贴山靠：地面撞击冲击环。 */
        IMPACT_RING(TIE_SHAN_KAO, 10, 16),
        /** 血崩步：步法残影短尾。 */
        DASH_AFTERIMAGE(XUE_BENG_BU, 12, 14),
        /** 逆脉护体：体表逆流纹。 */
        BODY_COUNTERFLOW(NI_MAI_HU_TI, 14, 60);

        final Identifier eventId;
        final int defaultCount;
        final int defaultLifetime;

        Form(Identifier eventId, int defaultCount, int defaultLifetime) {
            this.eventId = eventId;
            this.defaultCount = defaultCount;
            this.defaultLifetime = defaultLifetime;
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
        Form form = formFor(payload.eventId());
        if (form == null) {
            // 未登记 id：静默跳过（接线错误不该让渲染线程抛异常）。
            return;
        }
        switch (form) {
            case IMPACT_RING -> playImpactRing(client, world, payload);
            case DASH_AFTERIMAGE -> playDashAfterimage(client, world, payload);
            case BODY_COUNTERFLOW -> playBodyCounterflow(client, world, payload);
        }
    }

    /** 贴山靠：地面撞击冲击环。半径吃 strength，整圈自旋表达"碾压"。 */
    private static void playImpactRing(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = clampInt(payload.count().orElse(Form.IMPACT_RING.defaultCount), 4, 48);
        float[] color = rgb(payload.colorRgb().orElse(FAMILY_RGB));
        double strength = clamp(payload.strength().orElse(0.95), 0.0, 1.0);
        int lifetime = payload.durationTicks().orElse(Form.IMPACT_RING.defaultLifetime);
        double halfSize = 0.30 + 0.45 * strength;

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            BongGroundDecalParticle particle = new BongGroundDecalParticle(
                world,
                origin[0] + Math.cos(angle) * halfSize,
                // 贴地：以 caster 脚下为准（server origin 抬高了 1 格胸口高度）。
                origin[1] - 1.0 + 0.03,
                origin[2] + Math.sin(angle) * halfSize
            ).setDecalShape(halfSize, 0.03).setSpin(angle, 0.05);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.72f);
            particle.setMaxAgePublic(lifetime);
            if (BongParticles.lingqiRippleSprites != null) {
                particle.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /**
     * 血崩步：步法残影短尾。
     *
     * <p>关键取向——ribbon 沿 {@code direction} 的<b>反向</b>拖出，残影落在身后，
     * 才读得出"人往前窜了"。顺向拖尾会读成"往前喷气"，语义相反。
     */
    private static void playDashAfterimage(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        double[] dir = ZhenmaiPulsePlayer.normalizedDirection(payload.direction().orElse(null));
        int count = clampInt(payload.count().orElse(Form.DASH_AFTERIMAGE.defaultCount), 2, 48);
        float[] color = rgb(payload.colorRgb().orElse(FAMILY_RGB));
        double strength = clamp(payload.strength().orElse(0.85), 0.0, 1.0);
        int lifetime = payload.durationTicks().orElse(Form.DASH_AFTERIMAGE.defaultLifetime);
        double speed = 0.25;

        for (int i = 0; i < count; i++) {
            // 沿身高分布 + 轻微横向抖动，构成一束残影而不是一条线。
            double t = count == 1 ? 0.0 : (double) i / (count - 1);
            double lateral = (t - 0.5) * 0.5;
            BongRibbonParticle particle = new BongRibbonParticle(
                world,
                origin[0] - dir[0] * 0.15 + lateral * -dir[2],
                origin[1] - 0.4 + t * 1.1,
                origin[2] - dir[2] * 0.15 + lateral * dir[0],
                -dir[0] * speed,
                0.01,
                -dir[2] * speed
            );
            particle.setRibbonWidth(0.14 * (0.6 + strength), 0.02);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.68f);
            particle.setMaxAgePublic(lifetime);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 逆脉护体：体表逆流纹。双高度环 + 切向环绕，贴身而非外扩（护体不是攻击）。 */
    private static void playBodyCounterflow(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = clampInt(payload.count().orElse(Form.BODY_COUNTERFLOW.defaultCount), 4, 48);
        float[] color = rgb(payload.colorRgb().orElse(FAMILY_RGB));
        int lifetime = payload.durationTicks().orElse(Form.BODY_COUNTERFLOW.defaultLifetime);
        double radius = 0.55;
        double speed = 0.08;

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            double cos = Math.cos(angle);
            double sin = Math.sin(angle);
            // 奇偶分上下两环，读出"整个体表都在逆流"而不是只有一圈腰带。
            double heightOffset = (i % 2 == 0) ? -0.25 : 0.45;

            BongSpriteParticle particle = new BongSpriteParticle(
                world,
                origin[0] + cos * radius,
                origin[1] + heightOffset,
                origin[2] + sin * radius,
                // 切向环绕（逆时针，与崩拳的直线爆发形成对比）+ 缓慢上浮。
                -sin * speed,
                0.015,
                cos * speed
            );
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.8f);
            particle.setScalePublic(0.6f);
            particle.setMaxAgePublic(lifetime);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
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
