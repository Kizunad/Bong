package com.bong.client.atmosphere;

import com.bong.client.environment.EnvironmentFogCommand;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.state.SeasonState;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.state.ZoneState;
import com.bong.client.tsy.ExtractState;
import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.visual.particle.BongParticles;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.DustParticleEffect;
import net.minecraft.particle.ParticleEffect;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.math.Vec3d;
import net.minecraft.util.math.random.Random;
import org.joml.Vector3f;

import java.util.LinkedHashMap;
import java.util.Map;

public final class ZoneAtmosphereRenderer {
    private static final AshFootprintTracker FOOTPRINTS = new AshFootprintTracker();
    private static volatile ZoneAtmosphereProfileRegistry registry = ZoneAtmosphereProfileRegistry.loadDefault();
    private static volatile ZoneAtmosphereCommand currentCommand;
    private static volatile SeasonOverride seasonOverride;
    private static boolean bootstrapped;

    private ZoneAtmosphereRenderer() {
    }

    public static void bootstrap() {
        if (bootstrapped) {
            return;
        }
        bootstrapped = true;
        registry = ZoneAtmosphereProfileRegistry.loadDefault();
    }

    public static void update(MinecraftClient client, Vec3d playerPos) {
        if (client == null || client.world == null || client.player == null || playerPos == null) {
            currentCommand = null;
            FOOTPRINTS.clear();
            return;
        }

        ZoneState zoneState = BongHudStateStore.snapshot().zoneState();
        ExtractState extractState = ExtractStateStore.snapshot();
        long nowMillis = System.currentTimeMillis();
        ZoneAtmosphereContext context = ZoneAtmosphereContext
            .of(zoneState, SeasonStateStore.snapshot())
            .withCollapse(extractState.collapseRemainingTicks(nowMillis), extractState.collapseRemainingTicksAtStart());
        currentCommand = ZoneAtmospherePlanner.plan(registry, context, nowMillis);
        spawnAmbientParticles(client.world, client.player, currentCommand);
        spawnFootprintParticles(client.world, client.player, currentCommand);
    }

    public static ZoneAtmosphereCommand currentCommand() {
        return currentCommand;
    }

    public static EnvironmentFogCommand currentFogCommand() {
        ZoneAtmosphereCommand command = currentCommand;
        if (command == null) {
            return null;
        }
        return new EnvironmentFogCommand(
            command.fogStart(),
            command.fogEnd(),
            command.fogColorRgb(),
            command.skyTintRgb(),
            command.fogDensity()
        );
    }

    public static void setSeasonOverride(SeasonState.Phase phase, double progress) {
        seasonOverride = new SeasonOverride(
            phase == null ? SeasonState.Phase.SUMMER : phase,
            clamp01(progress)
        );
    }

    public static SeasonOverride currentSeasonOverrideForTests() {
        return seasonOverride;
    }

    public static void clearSeasonOverrideForTests() {
        seasonOverride = null;
    }

    public static void clear() {
        currentCommand = null;
        FOOTPRINTS.clear();
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }

    public static EnvironmentFogCommand mergeFogCommands(EnvironmentFogCommand environment, EnvironmentFogCommand atmosphere) {
        if (environment == null) {
            return atmosphere;
        }
        if (atmosphere == null) {
            return environment;
        }
        double totalDensity = Math.max(0.0001, environment.density() + atmosphere.density());
        double environmentWeight = environment.density() / totalDensity;
        return new EnvironmentFogCommand(
            Math.min(environment.fogStart(), atmosphere.fogStart()),
            Math.min(environment.fogEnd(), atmosphere.fogEnd()),
            ZoneBoundaryTransition.blendRgb(atmosphere.fogColorRgb(), environment.fogColorRgb(), environmentWeight),
            ZoneBoundaryTransition.blendRgb(atmosphere.skyColorRgb(), environment.skyColorRgb(), environmentWeight),
            Math.max(environment.density(), atmosphere.density())
        );
    }

    public static void reloadProfilesForTests(Map<String, String> jsonByZoneId) {
        registry = ZoneAtmosphereProfileRegistry.fromJson(jsonByZoneId == null ? Map.of() : new LinkedHashMap<>(jsonByZoneId));
        currentCommand = null;
        FOOTPRINTS.clear();
    }

    public static void resetForTests() {
        registry = ZoneAtmosphereProfileRegistry.loadDefault();
        currentCommand = null;
        seasonOverride = null;
        FOOTPRINTS.clear();
    }

    private static void spawnAmbientParticles(ClientWorld world, ClientPlayerEntity player, ZoneAtmosphereCommand command) {
        if (world == null || player == null || command == null || command.particles().isEmpty()) {
            return;
        }
        Random random = world.random;
        long tick = world.getTime();
        for (ZoneAtmosphereProfile.ParticleConfig particle : command.particles()) {
            int intervalTicks = Math.max(1, particle.intervalTicks());
            if (tick % intervalTicks != 0) {
                continue;
            }
            int count = Math.max(1, (int) Math.round(particle.density()));
            for (int i = 0; i < count; i++) {
                double x = player.getX() + (random.nextDouble() - 0.5) * 18.0;
                double y = player.getY() + 0.4 + random.nextDouble() * 5.0;
                double z = player.getZ() + (random.nextDouble() - 0.5) * 18.0;
                world.addParticle(
                    particleEffect(particle),
                    x,
                    y,
                    z,
                    particle.driftX(),
                    particle.driftY(),
                    particle.driftZ()
                );
            }
        }
    }

    private static void spawnFootprintParticles(ClientWorld world, ClientPlayerEntity player, ZoneAtmosphereCommand command) {
        if (world == null || player == null || command == null) {
            FOOTPRINTS.clear();
            return;
        }
        if (!command.deadZoneVisual()) {
            FOOTPRINTS.clear();
            return;
        }
        for (AshFootprintTracker.FootprintCommand footprint : FOOTPRINTS.onEntityStep(
            player.getId(),
            player.getPos(),
            world.getTime(),
            command
        )) {
            for (int i = 0; i < footprint.count(); i++) {
                world.addParticle(
                    new DustParticleEffect(rgbVector(footprint.tintRgb()), 0.8f),
                    player.getX(),
                    player.getY() + 0.04,
                    player.getZ(),
                    0.0,
                    0.025,
                    0.0
                );
            }
        }
    }

    private static ParticleEffect particleEffect(ZoneAtmosphereProfile.ParticleConfig particle) {
        return switch (particle.type()) {
            case "snow_grain" -> ParticleTypes.SNOWFLAKE;
            case "tribulation_spark" -> BongParticles.TRIBULATION_SPARK;
            case "lingqi_ripple" -> BongParticles.LINGQI_RIPPLE;
            case "enlightenment_dust" -> BongParticles.ENLIGHTENMENT_DUST;
            case "qi_aura" -> BongParticles.QI_AURA;
            default -> new DustParticleEffect(rgbVector(particle.tintRgb()), 1.0f);
        };
    }

    private static Vector3f rgbVector(int rgb) {
        return new Vector3f(
            ((rgb >>> 16) & 0xFF) / 255.0f,
            ((rgb >>> 8) & 0xFF) / 255.0f,
            (rgb & 0xFF) / 255.0f
        );
    }

    public record SeasonOverride(SeasonState.Phase phase, double progress) {
    }
}
