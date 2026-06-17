package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.Arrays;
import java.util.List;
import java.util.Objects;

public final class NpcParticleVfxPlayer implements VfxPlayer {
    public static final Identifier SKULL_FIEND_LOCKING = new Identifier("bong", "skull_fiend_locking");
    public static final Identifier SKULL_FIEND_TRAIL = new Identifier("bong", "skull_fiend_trail");
    public static final Identifier SKULL_FIEND_IMPACT = new Identifier("bong", "skull_fiend_impact");
    public static final Identifier SKULL_FIEND_STUNNED = new Identifier("bong", "skull_fiend_stunned");
    public static final Identifier HYBRID_FORMATION = new Identifier("bong", "vfx/hybrid_formation");
    public static final Identifier HYBRID_RAGE = new Identifier("bong", "vfx/hybrid_rage");
    public static final Identifier SUPPLY_COFFIN_EMERGE = new Identifier("bong", "supply_coffin_emerge");
    public static final Identifier SUPPLY_COFFIN_BREAK = new Identifier("bong", "supply_coffin_break");

    public static final List<Identifier> EVENT_IDS = Arrays.stream(Kind.values())
        .map(Kind::eventId)
        .toList();

    private final Kind kind;

    public NpcParticleVfxPlayer(Kind kind) {
        this.kind = Objects.requireNonNull(kind, "kind");
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        ParticleSpec spec = resolveSpec(payload, kind);
        if (spec.shape() == ParticleShape.RIBBON) {
            spawnRibbons(client, world, payload, spec);
        } else {
            spawnLines(client, world, payload, spec);
        }
    }

    static ParticleSpec resolveSpec(VfxEventPayload.SpawnParticle payload, Kind kind) {
        float[] rgb = GameplayVfxUtil.rgb(payload, kind.fallbackRgb());
        return new ParticleSpec(
            kind,
            kind.shape(),
            rgb[0],
            rgb[1],
            rgb[2],
            GameplayVfxUtil.count(payload, kind.fallbackCount(), 1, kind.maxCount()),
            GameplayVfxUtil.duration(payload, kind.fallbackDurationTicks()),
            GameplayVfxUtil.strength(payload, kind.fallbackStrength()),
            GameplayVfxUtil.direction(payload, kind.fallbackDirection())
        );
    }

    private static void spawnLines(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        ParticleSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + spec.kind().yOffset();
        double oz = payload.origin()[2];
        SpriteProvider provider = BongParticles.swordQiTrailSprites;
        for (int i = 0; i < spec.count(); i++) {
            double[] velocity = jitteredVelocity(world, spec.direction(), 0.06 + spec.strength() * 0.08);
            GameplayVfxUtil.spawnLine(
                client,
                world,
                provider,
                ox,
                oy,
                oz,
                velocity[0],
                velocity[1],
                velocity[2],
                spec.rgb(),
                0.55f + (float) spec.strength() * 0.35f,
                spec.maxAge(),
                spec.kind().lineHalfWidth()
            );
        }
    }

    private static void spawnRibbons(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        ParticleSpec spec
    ) {
        if (client == null || client.particleManager == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + spec.kind().yOffset();
        double oz = payload.origin()[2];
        SpriteProvider provider = BongParticles.vortexSpiralSprites;
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / Math.max(1, spec.count())
                + world.random.nextDouble() * 0.24;
            double radius = spec.kind().radius() * (0.55 + world.random.nextDouble() * 0.45);
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.45;
            double speed = 0.035 + spec.strength() * 0.055;
            BongRibbonParticle ribbon = new BongRibbonParticle(
                world,
                x,
                y,
                z,
                -Math.sin(angle) * speed,
                (world.random.nextDouble() - 0.4) * 0.03,
                Math.cos(angle) * speed
            );
            ribbon.setRibbonWidth(spec.kind().ribbonHeadWidth(), spec.kind().ribbonTailWidth());
            ribbon.setColor(spec.red(), spec.green(), spec.blue());
            ribbon.setAlphaPublic(0.45f + (float) spec.strength() * 0.35f);
            ribbon.setMaxAgePublic(spec.maxAge());
            if (provider != null) {
                ribbon.setSpritePublic(provider.getSprite(world.random));
            }
            client.particleManager.addParticle(ribbon);
        }
    }

    private static double[] jitteredVelocity(ClientWorld world, double[] direction, double speed) {
        return new double[] {
            direction[0] * speed + (world.random.nextDouble() - 0.5) * 0.09,
            direction[1] * speed + (world.random.nextDouble() - 0.25) * 0.06,
            direction[2] * speed + (world.random.nextDouble() - 0.5) * 0.09
        };
    }

    public enum Kind {
        SKULL_FIEND_LOCKING(
            NpcParticleVfxPlayer.SKULL_FIEND_LOCKING,
            ParticleShape.LINE,
            0xAA0022,
            18,
            30,
            0.9,
            18,
            0.75,
            0.035,
            0.0,
            0.0,
            0.0
        ),
        SKULL_FIEND_TRAIL(
            NpcParticleVfxPlayer.SKULL_FIEND_TRAIL,
            ParticleShape.LINE,
            0x31004A,
            14,
            20,
            0.75,
            24,
            0.65,
            0.028,
            0.0,
            0.0,
            0.0
        ),
        SKULL_FIEND_IMPACT(
            NpcParticleVfxPlayer.SKULL_FIEND_IMPACT,
            ParticleShape.LINE,
            0xF2F2FF,
            28,
            18,
            1.0,
            36,
            0.85,
            0.045,
            0.0,
            0.0,
            0.0
        ),
        SKULL_FIEND_STUNNED(
            NpcParticleVfxPlayer.SKULL_FIEND_STUNNED,
            ParticleShape.RIBBON,
            0x7C2BCB,
            16,
            40,
            0.75,
            24,
            0.9,
            0.0,
            0.75,
            0.10,
            0.018
        ),
        HYBRID_FORMATION(
            NpcParticleVfxPlayer.HYBRID_FORMATION,
            ParticleShape.RIBBON,
            0xA07058,
            24,
            36,
            0.9,
            48,
            0.95,
            0.0,
            1.15,
            0.11,
            0.02
        ),
        HYBRID_RAGE(
            NpcParticleVfxPlayer.HYBRID_RAGE,
            ParticleShape.LINE,
            0xFF4010,
            8,
            18,
            0.7,
            24,
            0.7,
            0.04,
            0.0,
            0.0,
            0.0
        ),
        SUPPLY_COFFIN_EMERGE(
            NpcParticleVfxPlayer.SUPPLY_COFFIN_EMERGE,
            ParticleShape.RIBBON,
            0xA08050,
            6,
            25,
            0.65,
            18,
            0.15,
            0.0,
            0.55,
            0.08,
            0.016
        ),
        SUPPLY_COFFIN_BREAK(
            NpcParticleVfxPlayer.SUPPLY_COFFIN_BREAK,
            ParticleShape.LINE,
            0x8B6914,
            12,
            15,
            0.8,
            28,
            0.45,
            0.035,
            0.0,
            0.0,
            0.0
        );

        private final Identifier eventId;
        private final ParticleShape shape;
        private final int fallbackRgb;
        private final int fallbackCount;
        private final int fallbackDurationTicks;
        private final double fallbackStrength;
        private final int maxCount;
        private final double yOffset;
        private final double lineHalfWidth;
        private final double radius;
        private final double ribbonHeadWidth;
        private final double ribbonTailWidth;

        Kind(
            Identifier eventId,
            ParticleShape shape,
            int fallbackRgb,
            int fallbackCount,
            int fallbackDurationTicks,
            double fallbackStrength,
            int maxCount,
            double yOffset,
            double lineHalfWidth,
            double radius,
            double ribbonHeadWidth,
            double ribbonTailWidth
        ) {
            this.eventId = eventId;
            this.shape = shape;
            this.fallbackRgb = fallbackRgb;
            this.fallbackCount = fallbackCount;
            this.fallbackDurationTicks = fallbackDurationTicks;
            this.fallbackStrength = fallbackStrength;
            this.maxCount = maxCount;
            this.yOffset = yOffset;
            this.lineHalfWidth = lineHalfWidth;
            this.radius = radius;
            this.ribbonHeadWidth = ribbonHeadWidth;
            this.ribbonTailWidth = ribbonTailWidth;
        }

        public Identifier eventId() {
            return eventId;
        }

        ParticleShape shape() {
            return shape;
        }

        int fallbackRgb() {
            return fallbackRgb;
        }

        int fallbackCount() {
            return fallbackCount;
        }

        int fallbackDurationTicks() {
            return fallbackDurationTicks;
        }

        double fallbackStrength() {
            return fallbackStrength;
        }

        int maxCount() {
            return maxCount;
        }

        double yOffset() {
            return yOffset;
        }

        double lineHalfWidth() {
            return lineHalfWidth;
        }

        double radius() {
            return radius;
        }

        double ribbonHeadWidth() {
            return ribbonHeadWidth;
        }

        double ribbonTailWidth() {
            return ribbonTailWidth;
        }

        double[] fallbackDirection() {
            return new double[] { 0.0, 0.35, 1.0 };
        }
    }

    enum ParticleShape {
        LINE,
        RIBBON
    }

    record ParticleSpec(
        Kind kind,
        ParticleShape shape,
        float red,
        float green,
        float blue,
        int count,
        int maxAge,
        double strength,
        double[] direction
    ) {
        float[] rgb() {
            return new float[] { red, green, blue };
        }
    }
}
