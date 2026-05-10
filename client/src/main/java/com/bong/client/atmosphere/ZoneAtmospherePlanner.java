package com.bong.client.atmosphere;

import com.bong.client.state.SeasonState;
import com.bong.client.state.ZoneState;

import java.util.ArrayList;
import java.util.List;

public final class ZoneAtmospherePlanner {
    private static final int DEAD_FOG_RGB = 0xFFFFFF;
    private static final int DEAD_SKY_RGB = 0xF0F0F0;
    private static final int NEGATIVE_VIGNETTE_RGB = 0x330033;
    private static final int COLLAPSE_BLACK_RGB = 0x000000;

    private ZoneAtmospherePlanner() {
    }

    public static ZoneAtmosphereCommand plan(ZoneAtmosphereProfileRegistry registry, ZoneAtmosphereContext context, long nowMillis) {
        ZoneAtmosphereContext safeContext = context == null
            ? ZoneAtmosphereContext.of(ZoneState.empty(), null)
            : context;
        ZoneState zoneState = safeContext.zoneState();
        if (zoneState.isEmpty()) {
            return null;
        }

        ZoneAtmosphereProfile baseProfile = registry == null
            ? ZoneAtmosphereProfileRegistry.loadDefault().forZone(zoneState.zoneId())
            : registry.forZone(zoneState.zoneId());
        if (baseProfile == null) {
            return null;
        }
        if (safeContext.boundaryTarget() != null) {
            baseProfile = ZoneBoundaryTransition.blend(baseProfile, safeContext.boundaryTarget(), safeContext.boundaryProgress());
        }

        boolean deadZone = deadZone(zoneState);
        boolean negativeZone = zoneState.negativeSpiritQi();
        ZoneAtmosphereProfile profile = deadZone
            ? deadZoneProfile(baseProfile.zoneId(), baseProfile.ambientRecipeId())
            : applySeason(baseProfile, safeContext.seasonState(), nowMillis);
        profile = applyTsyProfile(profile, zoneState, safeContext.tsyTier());

        double desaturation = deadZone ? 0.5 : 0.0;
        double vignette = negativeZone ? Math.min(1.0, Math.abs(zoneState.spiritQiRaw()) * 0.3) : 0.0;
        double distortion = negativeZone ? Math.min(0.65, Math.abs(zoneState.spiritQiRaw()) * 0.12) : 0.0;
        double breathing = tsyTier(zoneState, safeContext.tsyTier()) >= 7 ? 0.005 : 0.0;
        double cameraShake = 0.0;
        boolean hardClip = deadZone;

        CollapseVisual collapse = collapseVisual(safeContext.collapseRemainingTicks(), safeContext.collapseTotalTicks());
        if (collapse.active()) {
            profile = profile.withFogAndSky(
                ZoneBoundaryTransition.blendRgb(profile.fogColorRgb(), COLLAPSE_BLACK_RGB, collapse.blacken()),
                Math.max(profile.fogDensity(), collapse.fogDensity()),
                ZoneBoundaryTransition.blendRgb(profile.skyTintRgb(), COLLAPSE_BLACK_RGB, collapse.blacken())
            );
            vignette = Math.max(vignette, collapse.vignette());
            cameraShake = collapse.cameraShake();
            hardClip = hardClip || collapse.blacken() >= 1.0;
        }

        return new ZoneAtmosphereCommand(
            profile.zoneId(),
            profile.fogColorRgb(),
            profile.fogDensity(),
            fogStart(profile.fogDensity(), hardClip, zoneState, safeContext.tsyTier()),
            fogEnd(profile.fogDensity(), hardClip, zoneState, safeContext.tsyTier()),
            profile.skyTintRgb(),
            profile.ambientParticles(),
            profile.entryTransitionFx(),
            profile.ambientRecipeId(),
            desaturation,
            vignette,
            NEGATIVE_VIGNETTE_RGB,
            distortion,
            breathing,
            cameraShake,
            hardClip,
            deadZone,
            negativeZone
        );
    }

    static ZoneAtmosphereProfile applySeason(ZoneAtmosphereProfile profile, SeasonState seasonState, long nowMillis) {
        if (profile == null || seasonState == null) {
            return profile;
        }
        return switch (seasonState.phase()) {
            case SUMMER -> profile.withFogAndSky(
                warmShift(profile.fogColorRgb(), 0.08),
                profile.fogDensity() * 0.8,
                warmShift(profile.skyTintRgb(), 0.18)
            ).withParticles(scaleParticleSpeed(profile.ambientParticles(), 1.3, false));
            case WINTER -> profile.withFogAndSky(
                coolShift(profile.fogColorRgb(), 0.12),
                profile.fogDensity() * 1.3,
                coolShift(profile.skyTintRgb(), 0.20)
            ).withParticles(addWinterSnow(profile.ambientParticles()));
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> {
                double pulse = ((nowMillis / 500L) & 1L) == 0L ? -0.1 : 0.1;
                yield profile.withFogAndSky(
                    ZoneBoundaryTransition.blendRgb(profile.fogColorRgb(), 0x6A5D78, 0.18),
                    profile.fogDensity() + pulse,
                    ZoneBoundaryTransition.blendRgb(profile.skyTintRgb(), 0x7A6688, 0.22)
                ).withParticles(addTideTurnTurbulence(profile.ambientParticles()));
            }
        };
    }

    private static ZoneAtmosphereProfile applyTsyProfile(ZoneAtmosphereProfile profile, ZoneState zoneState, int explicitTier) {
        int tier = tsyTier(zoneState, explicitTier);
        if (tier <= 0) {
            return profile;
        }
        if (tier <= 3) {
            return profile.withFogAndSky(0x404050, 0.30, 0x202030);
        }
        if (tier <= 6) {
            return profile.withFogAndSky(0x252530, 0.60, 0x15151C);
        }
        return profile.withFogAndSky(0x101015, 0.90, 0x08080C).withParticles(addDeepTsySparks(profile.ambientParticles()));
    }

    private static int tsyTier(ZoneState zoneState, int explicitTier) {
        if (explicitTier > 0) {
            return explicitTier;
        }
        String zoneId = zoneState == null ? "" : zoneState.zoneId().toLowerCase(java.util.Locale.ROOT);
        if (zoneId.startsWith("tsy") || zoneId.contains("tianshuiyao")) {
            return Math.max(1, zoneState.dangerLevel() + 1);
        }
        return 0;
    }

    private static boolean deadZone(ZoneState zoneState) {
        if (zoneState == null || zoneState.isEmpty()) {
            return false;
        }
        return zoneState.collapsed() || (!zoneState.negativeSpiritQi() && Math.abs(zoneState.spiritQiRaw()) <= 0.0001);
    }

    private static ZoneAtmosphereProfile deadZoneProfile(String zoneId, String ambientRecipeId) {
        return new ZoneAtmosphereProfile(
            zoneId,
            DEAD_FOG_RGB,
            1.0,
            List.of(new ZoneAtmosphereProfile.ParticleConfig("cloud256_dust", 0x808080, 0.2, 0.0, -0.002, 0.0, 40)),
            DEAD_SKY_RGB,
            ZoneAtmosphereProfile.TransitionFx.FADE,
            ambientRecipeId
        );
    }

    private static List<ZoneAtmosphereProfile.ParticleConfig> scaleParticleSpeed(
        List<ZoneAtmosphereProfile.ParticleConfig> particles,
        double factor,
        boolean verticalOnly
    ) {
        List<ZoneAtmosphereProfile.ParticleConfig> out = new ArrayList<>();
        for (ZoneAtmosphereProfile.ParticleConfig particle : particles) {
            out.add(particle.withDrift(
                verticalOnly ? particle.driftX() : particle.driftX() * factor,
                particle.driftY() * factor,
                verticalOnly ? particle.driftZ() : particle.driftZ() * factor
            ));
        }
        return out;
    }

    private static List<ZoneAtmosphereProfile.ParticleConfig> addWinterSnow(List<ZoneAtmosphereProfile.ParticleConfig> particles) {
        List<ZoneAtmosphereProfile.ParticleConfig> out = new ArrayList<>(particles);
        out.add(new ZoneAtmosphereProfile.ParticleConfig("snow_grain", 0xFFFFFF, 1.0, 0.0, -0.03, 0.0, 10));
        return out;
    }

    private static List<ZoneAtmosphereProfile.ParticleConfig> addTideTurnTurbulence(List<ZoneAtmosphereProfile.ParticleConfig> particles) {
        List<ZoneAtmosphereProfile.ParticleConfig> out = new ArrayList<>(particles);
        out.add(new ZoneAtmosphereProfile.ParticleConfig("lingqi_turbulence", 0xAA88FF, 0.45, 0.04, 0.01, -0.02, 60));
        return out;
    }

    private static List<ZoneAtmosphereProfile.ParticleConfig> addDeepTsySparks(List<ZoneAtmosphereProfile.ParticleConfig> particles) {
        List<ZoneAtmosphereProfile.ParticleConfig> out = new ArrayList<>(particles);
        out.add(new ZoneAtmosphereProfile.ParticleConfig("tribulation_spark", 0x6A44AA, 0.12, 0.0, 0.0, 0.0, 200));
        return out;
    }

    private static int warmShift(int rgb, double t) {
        return ZoneBoundaryTransition.blendRgb(rgb, 0xFFD080, t);
    }

    private static int coolShift(int rgb, double t) {
        return ZoneBoundaryTransition.blendRgb(rgb, 0xDDEEFF, t);
    }

    private static double fogStart(double density, boolean hardClip, ZoneState zoneState, int explicitTsyTier) {
        if (hardClip && deadZone(zoneState)) {
            return 0.0;
        }
        int tier = tsyTier(zoneState, explicitTsyTier);
        if (tier >= 7) {
            return 2.0;
        }
        if (tier >= 4) {
            return 6.0;
        }
        if (tier >= 1) {
            return 12.0;
        }
        return Math.max(0.0, 32.0 - ZoneAtmosphereProfile.clamp01(density) * 24.0);
    }

    private static double fogEnd(double density, boolean hardClip, ZoneState zoneState, int explicitTsyTier) {
        if (hardClip && deadZone(zoneState)) {
            return 150.0;
        }
        int tier = tsyTier(zoneState, explicitTsyTier);
        if (tier >= 7) {
            return 8.0;
        }
        if (tier >= 4) {
            return 20.0;
        }
        if (tier >= 1) {
            return 50.0;
        }
        return Math.max(24.0, 160.0 - ZoneAtmosphereProfile.clamp01(density) * 112.0);
    }

    private static CollapseVisual collapseVisual(int remainingTicks, int totalTicks) {
        if (remainingTicks <= 0 || totalTicks <= 0) {
            return CollapseVisual.inactive();
        }
        double remainingSeconds = remainingTicks / 20.0;
        if (remainingSeconds <= 10.0) {
            return new CollapseVisual(true, 1.0, 1.0, 0.95, 0.5);
        }
        if (remainingSeconds <= 30.0) {
            double t = (30.0 - remainingSeconds) / 20.0;
            return new CollapseVisual(true, 0.75 + t * 0.25, 0.7 + t * 0.3, 0.45 + t * 0.25, 0.18);
        }
        if (remainingSeconds <= 60.0) {
            double t = (60.0 - remainingSeconds) / 30.0;
            return new CollapseVisual(true, t * 0.75, 0.65 + t * 0.25, t * 0.45, 0.0);
        }
        return CollapseVisual.inactive();
    }

    private record CollapseVisual(
        boolean active,
        double blacken,
        double fogDensity,
        double vignette,
        double cameraShake
    ) {
        static CollapseVisual inactive() {
            return new CollapseVisual(false, 0.0, 0.0, 0.0, 0.0);
        }
    }
}
