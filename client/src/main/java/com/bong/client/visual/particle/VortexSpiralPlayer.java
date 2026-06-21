package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** Spawns several inward ribbon trails around a woliu-v2 low-pressure point. */
public final class VortexSpiralPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "vortex_spiral");
    // AV 差异化：woliu 基础 5 招专属 particle event_id（复用本 player 的 vortexSpiralSprites，无新贴图）。
    // 必须与 server visual_for() 中各招 particle_id 精确逐字符一致。
    public static final Identifier HOLD_SUSTAIN = new Identifier("bong", "woliu_hold_sustain");
    public static final Identifier BURST_POP = new Identifier("bong", "woliu_burst_pop");
    public static final Identifier MOUTH_FUNNEL = new Identifier("bong", "woliu_mouth_funnel");
    public static final Identifier PULL_DRAG = new Identifier("bong", "woliu_pull_drag");
    public static final Identifier HEART_FIELD = new Identifier("bong", "woliu_heart_field");
    public static final Identifier VACUUM_PALM = new Identifier("bong", "woliu_vacuum_palm_spiral");
    public static final Identifier VORTEX_SHIELD = new Identifier("bong", "woliu_vortex_shield_sphere");
    public static final Identifier VACUUM_LOCK = new Identifier("bong", "woliu_vacuum_lock_cage");
    public static final Identifier VORTEX_RESONANCE = new Identifier("bong", "woliu_vortex_resonance_field");
    public static final Identifier TURBULENCE_BURST = new Identifier("bong", "woliu_turbulence_burst_wave");
    // plan-woliu-path-v1：虚蚀路径 5 招式 — particle IDs must match visual_for() in server skills.rs exactly
    public static final Identifier VORTEX_AMBIENT = new Identifier("bong", "vortex_ambient");
    public static final Identifier VOID_SPHERE = new Identifier("bong", "woliu_void_sphere");
    public static final Identifier SWALLOWING_SPIRAL = new Identifier("bong", "woliu_swallowing_spiral");
    public static final Identifier ECHO_RIPPLE = new Identifier("bong", "woliu_echo_ripple");
    public static final Identifier VOID_CORE_COLLAPSE = new Identifier("bong", "woliu_void_core_collapse");

    private static final int DEFAULT_COUNT = 10;
    private static final int FALLBACK_RGB = 0x201832;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;
        EffectSpec spec = effectSpec(payload);

        if (spec.route() == Route.RESONANCE_FIELD) {
            playResonanceField(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.TURBULENCE_BURST) {
            playTurbulenceBurst(client, world, payload, spec);
            return;
        }
        // plan-woliu-path-v1：虚蚀路径路由
        if (spec.route() == Route.VORTEX_AMBIENT) {
            playVortexAmbient(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.VOID_SPHERE) {
            playVoidSphere(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.SWALLOWING_SPIRAL) {
            playSwallowingSpiral(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.ECHO_RIPPLE) {
            playEchoRipple(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.VOID_CORE_COLLAPSE) {
            playVoidCoreCollapse(client, world, payload, spec);
            return;
        }
        // AV 差异化：woliu 基础 5 招各自专属形态
        if (spec.route() == Route.HOLD_SUSTAIN) {
            playHoldSustain(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.BURST_POP) {
            playBurstPop(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.MOUTH_FUNNEL) {
            playMouthFunnel(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.PULL_DRAG) {
            playPullDrag(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.HEART_FIELD) {
            playHeartField(client, world, payload, spec);
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.0;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);

        for (int i = 0; i < spec.count(); i++) {
            double angle = (Math.PI * 2.0 * i / spec.count()) + world.random.nextDouble() * 0.35;
            double radius = 0.35 + world.random.nextDouble() * 0.65;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.45;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                -Math.sin(angle) * 0.035,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.cos(angle) * 0.035,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.055 + spec.strength() * 0.08);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge());
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playResonanceField(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.95;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);

        for (int i = 0; i < spec.count(); i++) {
            int ring = i % 3;
            double ringRatio = 0.34 + ring * 0.28;
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.22;
            double radius = spec.radius() * ringRatio + (world.random.nextDouble() - 0.5) * 0.35;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + Math.sin(angle * 2.0 + ring) * 0.32 + (world.random.nextDouble() - 0.5) * 0.18;
            double tangent = 0.055 + spec.strength() * 0.045 + ring * 0.012;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                -Math.sin(angle) * tangent,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.cos(angle) * tangent,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.09 + spec.strength() * 0.09 + ring * 0.015);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 4)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playTurbulenceBurst(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.75;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);

        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.16;
            double x = ox + Math.cos(angle) * spec.radius();
            double z = oz + Math.sin(angle) * spec.radius();
            double y = oy + (world.random.nextDouble() - 0.5) * 0.5;
            double speed = 0.10 + spec.strength() * 0.08 + world.random.nextDouble() * 0.04;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                Math.cos(angle) * speed,
                (world.random.nextDouble() - 0.2) * 0.025,
                Math.sin(angle) * speed,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.02 + spec.strength() * 0.04);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** AmbientVortex：持续低频旋涡，深紫慢速收缩，粒子绕轴慢转。 */
    private static void playVortexAmbient(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.8;
        double oz = payload.origin()[2];
        // fallback: deep purple if no color in payload
        int rgb = payload.colorRgb().orElse(0x2A0A3C);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            double angle = (Math.PI * 2.0 * i / spec.count()) + world.random.nextDouble() * 0.5;
            double radius = 0.4 + world.random.nextDouble() * 0.5;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.6;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                -Math.sin(angle) * 0.018,
                (world.random.nextDouble() - 0.5) * 0.008,
                Math.cos(angle) * 0.018,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.025 + spec.strength() * 0.03);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() + world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** VoidVortex：虚空球体，多环密排粒子，偏蓝灰。 */
    private static void playVoidSphere(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.0;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x1C2B38);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            int ring = i % 4;
            double ringRatio = 0.25 + ring * 0.22;
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.18;
            double radius = spec.radius() * ringRatio + (world.random.nextDouble() - 0.5) * 0.25;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + Math.sin(angle * 2.0 + ring * 0.7) * 0.45 + (world.random.nextDouble() - 0.5) * 0.2;
            double tangent = 0.04 + spec.strength() * 0.035 + ring * 0.008;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                -Math.sin(angle) * tangent,
                (world.random.nextDouble() - 0.5) * 0.010,
                Math.cos(angle) * tangent,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.07 + spec.strength() * 0.06 + ring * 0.010);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 5)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** SwallowingVortex：吞噬螺旋，大半径向心拉扯，偏暗红。 */
    private static void playSwallowingSpiral(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.9;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x3D0A0A);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            int ring = i % 3;
            double ringRatio = 0.30 + ring * 0.32;
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.25;
            double radius = spec.radius() * ringRatio + (world.random.nextDouble() - 0.5) * 0.4;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.55;
            // inward velocity — swallowing effect
            double inward = -(0.06 + spec.strength() * 0.05 + ring * 0.01);
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * inward,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.sin(angle) * inward,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.08 + spec.strength() * 0.08 + ring * 0.012);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 4)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** VortexEcho：余震涟漪，轻量环形脉冲，偏冷青。 */
    private static void playEchoRipple(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.05;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x0A2830);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.20;
            double radius = spec.radius() + (world.random.nextDouble() - 0.5) * 0.3;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            // flat ripple — minimal vertical spread
            double y = oy + (world.random.nextDouble() - 0.5) * 0.15;
            double outward = 0.03 + spec.strength() * 0.025;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * outward,
                (world.random.nextDouble() - 0.5) * 0.006,
                Math.sin(angle) * outward,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.04 + spec.strength() * 0.03);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** VoidCore：虚核崩塌，高密度中心内爆，偏墨色。 */
    private static void playVoidCoreCollapse(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.85;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x0A0A12);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.12;
            double radius = spec.radius() + (world.random.nextDouble() - 0.5) * 0.2;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.4;
            double speed = 0.12 + spec.strength() * 0.10 + world.random.nextDouble() * 0.05;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * speed,
                (world.random.nextDouble() - 0.2) * 0.030,
                Math.sin(angle) * speed,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.03 + spec.strength() * 0.05);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 持涡 Hold：维持伞——稀疏粒子绕轴慢转，长驻不收缩，深蓝静态感。 */
    private static void playHoldSustain(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.1;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);
        for (int i = 0; i < spec.count(); i++) {
            double angle = (Math.PI * 2.0 * i / spec.count()) + world.random.nextDouble() * 0.3;
            // 稳定的伞形半径环（维持感：半径波动小）
            double radius = 0.75 + world.random.nextDouble() * 0.25;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.25;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                -Math.sin(angle) * 0.022,
                (world.random.nextDouble() - 0.5) * 0.005,
                Math.cos(angle) * 0.022,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.03 + spec.strength() * 0.025);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() + world.random.nextInt(Math.max(1, spec.maxAge() / 4)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 瞬涡 Burst：200ms 弹反——短促向外爆开 pop，单帧密集高速，明亮蓝。 */
    private static void playBurstPop(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.0;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.14;
            double radius = 0.25 + world.random.nextDouble() * 0.2;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.3;
            // 强烈向外弹出速度，体现弹反瞬发
            double speed = 0.16 + spec.strength() * 0.12 + world.random.nextDouble() * 0.05;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * speed,
                (world.random.nextDouble() - 0.2) * 0.04,
                Math.sin(angle) * speed,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.02 + spec.strength() * 0.02);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            // 极短寿命：弹反一瞬即灭
            particle.setMaxAgePublic(Math.max(4, spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 2))));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 涡口 Mouth：远程点按——锥形漏斗向心收口，粒子高处向下汇聚，暗冷蓝。 */
    private static void playMouthFunnel(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.4;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.22;
            // 漏斗：粒子从高处大半径向下小半径收口
            double t = world.random.nextDouble();
            double radius = (0.3 + t * 0.9);
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + t * 0.6;
            // 向心+下沉速度
            double inward = -(0.05 + spec.strength() * 0.04);
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * inward,
                -0.03 - world.random.nextDouble() * 0.02,
                Math.sin(angle) * inward,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.06 + spec.strength() * 0.04);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 4)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 涡引 Pull：拉拽——大半径粒子向中心强力拖尾收拢，紫调。 */
    private static void playPullDrag(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.95;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.28;
            // 起始大半径，强向心速度——拉拽收拢感
            double radius = 1.0 + world.random.nextDouble() * 0.7;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.4;
            double inward = -(0.10 + spec.strength() * 0.08);
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * inward,
                (world.random.nextDouble() - 0.5) * 0.01,
                Math.sin(angle) * inward,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.05 + spec.strength() * 0.05);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 涡心 Heart：山谷级强制断经——大范围多环强压场，缓慢翻涌，深黑紫。 */
    private static void playHeartField(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.9;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);
        for (int i = 0; i < spec.count(); i++) {
            int ring = i % 3;
            double ringRatio = 0.4 + ring * 0.35;
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.2;
            double radius = spec.radius() * ringRatio + (world.random.nextDouble() - 0.5) * 0.4;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + Math.sin(angle * 1.5 + ring) * 0.4 + (world.random.nextDouble() - 0.5) * 0.25;
            double tangent = 0.045 + spec.strength() * 0.05 + ring * 0.01;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                -Math.sin(angle) * tangent,
                (world.random.nextDouble() - 0.5) * 0.014,
                Math.cos(angle) * tangent,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.06 + spec.strength() * 0.07 + ring * 0.012);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 5)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static float[] rgb(VfxEventPayload.SpawnParticle payload) {
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
    }

    static EffectSpec effectSpec(VfxEventPayload.SpawnParticle payload) {
        if (VORTEX_RESONANCE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.8));
            return new EffectSpec(
                Route.RESONANCE_FIELD,
                clamp(payload.count().orElse(48), 24, 96),
                clamp(payload.durationTicks().orElse(80), 30, 120),
                strength,
                2.2 + strength * 3.8,
                Math.min(0.9, 0.48 + strength * 0.34),
                0.12 + strength * 0.05,
                0.018
            );
        }
        if (TURBULENCE_BURST.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.9));
            return new EffectSpec(
                Route.TURBULENCE_BURST,
                clamp(payload.count().orElse(64), 24, 96),
                clamp(payload.durationTicks().orElse(44), 18, 80),
                strength,
                0.6 + strength * 0.7,
                Math.min(0.92, 0.55 + strength * 0.32),
                0.14 + strength * 0.04,
                0.02
            );
        }
        // plan-woliu-path-v1：虚蚀路径 5 招式 effectSpec（色相/形状各异化）
        // AmbientVortex：持续低频旋涡，颜色偏深紫，慢速收缩
        if (VORTEX_AMBIENT.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.6));
            return new EffectSpec(
                Route.VORTEX_AMBIENT,
                clamp(payload.count().orElse(16), 8, 48),
                clamp(payload.durationTicks().orElse(60), 30, 120),
                strength,
                0.0,
                Math.min(0.75, 0.38 + strength * 0.32),
                0.0,
                0.0
            );
        }
        // VoidVortex：虚空球体，较高粒子密度，偏蓝灰色
        if (VOID_SPHERE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.85));
            return new EffectSpec(
                Route.VOID_SPHERE,
                clamp(payload.count().orElse(36), 16, 72),
                clamp(payload.durationTicks().orElse(55), 20, 100),
                strength,
                1.8 + strength * 2.0,
                Math.min(0.88, 0.52 + strength * 0.3),
                0.10 + strength * 0.04,
                0.016
            );
        }
        // SwallowingVortex：吞噬螺旋，半径大，向心拉扯感强，偏暗红
        if (SWALLOWING_SPIRAL.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.9));
            return new EffectSpec(
                Route.SWALLOWING_SPIRAL,
                clamp(payload.count().orElse(52), 24, 96),
                clamp(payload.durationTicks().orElse(70), 30, 120),
                strength,
                3.0 + strength * 2.5,
                Math.min(0.90, 0.50 + strength * 0.34),
                0.13 + strength * 0.05,
                0.019
            );
        }
        // VortexEcho：余震涟漪，轻量环形脉冲，偏冷青色
        if (ECHO_RIPPLE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.7));
            return new EffectSpec(
                Route.ECHO_RIPPLE,
                clamp(payload.count().orElse(28), 12, 56),
                clamp(payload.durationTicks().orElse(38), 15, 80),
                strength,
                1.5 + strength * 1.8,
                Math.min(0.82, 0.44 + strength * 0.32),
                0.09 + strength * 0.03,
                0.014
            );
        }
        // VoidCore：虚核崩塌，最高强度，致密中心球爆炸，偏墨色
        if (VOID_CORE_COLLAPSE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(1.0));
            return new EffectSpec(
                Route.VOID_CORE_COLLAPSE,
                clamp(payload.count().orElse(72), 32, 96),
                clamp(payload.durationTicks().orElse(50), 20, 90),
                strength,
                0.8 + strength * 0.9,
                Math.min(0.95, 0.60 + strength * 0.30),
                0.16 + strength * 0.04,
                0.022
            );
        }
        // AV 差异化：woliu 基础 5 招 effectSpec（形态/强度/半径各异）
        if (HOLD_SUSTAIN.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.5));
            return new EffectSpec(
                Route.HOLD_SUSTAIN,
                clamp(payload.count().orElse(12), 6, 32),
                clamp(payload.durationTicks().orElse(70), 30, 120),
                strength,
                0.0,
                Math.min(0.7, 0.40 + strength * 0.28),
                0.0,
                0.0
            );
        }
        if (BURST_POP.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.8));
            return new EffectSpec(
                Route.BURST_POP,
                clamp(payload.count().orElse(28), 12, 56),
                clamp(payload.durationTicks().orElse(14), 6, 30),
                strength,
                0.0,
                Math.min(0.95, 0.60 + strength * 0.30),
                0.0,
                0.0
            );
        }
        if (MOUTH_FUNNEL.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.65));
            return new EffectSpec(
                Route.MOUTH_FUNNEL,
                clamp(payload.count().orElse(20), 10, 48),
                clamp(payload.durationTicks().orElse(48), 20, 90),
                strength,
                0.0,
                Math.min(0.82, 0.46 + strength * 0.30),
                0.0,
                0.0
            );
        }
        if (PULL_DRAG.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.7));
            return new EffectSpec(
                Route.PULL_DRAG,
                clamp(payload.count().orElse(24), 12, 56),
                clamp(payload.durationTicks().orElse(30), 15, 70),
                strength,
                0.0,
                Math.min(0.86, 0.48 + strength * 0.32),
                0.0,
                0.0
            );
        }
        if (HEART_FIELD.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.9));
            return new EffectSpec(
                Route.HEART_FIELD,
                clamp(payload.count().orElse(40), 18, 72),
                clamp(payload.durationTicks().orElse(100), 40, 120),
                strength,
                2.8 + strength * 3.2,
                Math.min(0.92, 0.52 + strength * 0.34),
                0.12 + strength * 0.05,
                0.018
            );
        }
        double strength = clamp01(payload.strength().orElse(0.75));
        return new EffectSpec(
            Route.SPIRAL,
            clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 1, 64),
            clamp(payload.durationTicks().orElse(42), 1, 120),
            strength,
            0.0,
            Math.max(0.35, Math.min(0.95, 0.45 + strength * 0.5)),
            0.0,
            0.0
        );
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private static double clamp01(double value) {
        return Math.max(0.0, Math.min(1.0, value));
    }

    enum Route {
        SPIRAL,
        RESONANCE_FIELD,
        TURBULENCE_BURST,
        // plan-woliu-path-v1：虚蚀路径 5 招式路由
        VORTEX_AMBIENT,
        VOID_SPHERE,
        SWALLOWING_SPIRAL,
        ECHO_RIPPLE,
        VOID_CORE_COLLAPSE,
        // AV 差异化：woliu 基础 5 招路由
        HOLD_SUSTAIN,
        BURST_POP,
        MOUTH_FUNNEL,
        PULL_DRAG,
        HEART_FIELD
    }

    record EffectSpec(
        Route route,
        int count,
        int maxAge,
        double strength,
        double radius,
        double alpha,
        double ribbonWidth,
        double ribbonEndWidth
    ) {}
}
