package com.bong.client.atmosphere;

import com.bong.client.BongClient;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class ZoneAtmosphereProfileRegistry {
    public static final List<String> REQUIRED_PROFILE_IDS = List.of(
        "spawn_plain",
        "qingyun_peaks",
        "blood_valley",
        "spring_marsh",
        "north_wastes",
        "wilderness",
        "dark_cavern",
        "tsy"
    );

    private final Map<String, ZoneAtmosphereProfile> profiles;

    private ZoneAtmosphereProfileRegistry(Map<String, ZoneAtmosphereProfile> profiles) {
        this.profiles = Map.copyOf(profiles);
    }

    public static ZoneAtmosphereProfileRegistry loadDefault() {
        Map<String, ZoneAtmosphereProfile> loaded = new LinkedHashMap<>();
        for (String id : REQUIRED_PROFILE_IDS) {
            ZoneAtmosphereProfile profile = loadClasspathProfile(id);
            if (profile != null) {
                loaded.put(profile.zoneId(), profile);
            }
        }
        if (loaded.isEmpty()) {
            return fallbackDefaults();
        }
        fallbackDefaults().profiles.forEach(loaded::putIfAbsent);
        return new ZoneAtmosphereProfileRegistry(loaded);
    }

    public static ZoneAtmosphereProfileRegistry fromJson(Map<String, String> jsonByZoneId) {
        Map<String, ZoneAtmosphereProfile> loaded = new LinkedHashMap<>();
        if (jsonByZoneId != null) {
            for (Map.Entry<String, String> entry : jsonByZoneId.entrySet()) {
                ZoneAtmosphereProfileParser.ParseResult result =
                    ZoneAtmosphereProfileParser.parse(entry.getValue(), entry.getKey());
                if (!result.ok()) {
                    throw new IllegalArgumentException(entry.getKey() + ": " + result.error());
                }
                loaded.put(result.profile().zoneId(), result.profile());
            }
        }
        fallbackDefaults().profiles.forEach(loaded::putIfAbsent);
        return new ZoneAtmosphereProfileRegistry(loaded);
    }

    public ZoneAtmosphereProfile forZone(String zoneId) {
        String normalized = normalizeZoneId(zoneId);
        ZoneAtmosphereProfile direct = profiles.get(normalized);
        if (direct != null) {
            return direct;
        }
        if (normalized.startsWith("tsy") || normalized.contains("tianshuiyao")) {
            return profiles.get("tsy");
        }
        return profiles.getOrDefault("wilderness", profiles.values().stream().findFirst().orElse(null));
    }

    public boolean hasProfile(String zoneId) {
        return profiles.containsKey(normalizeZoneId(zoneId));
    }

    public Map<String, ZoneAtmosphereProfile> profiles() {
        return profiles;
    }

    private static ZoneAtmosphereProfile loadClasspathProfile(String id) {
        String path = "assets/bong/atmosphere/" + id + ".json";
        try (InputStream stream = ZoneAtmosphereProfileRegistry.class.getClassLoader().getResourceAsStream(path)) {
            if (stream == null) {
                return null;
            }
            String json = new String(stream.readAllBytes(), StandardCharsets.UTF_8);
            ZoneAtmosphereProfileParser.ParseResult result = ZoneAtmosphereProfileParser.parse(json, id);
            if (result.ok()) {
                return result.profile();
            }
            BongClient.LOGGER.warn("Ignoring atmosphere profile {}: {}", path, result.error());
            return null;
        } catch (IOException ex) {
            BongClient.LOGGER.warn("Failed to load atmosphere profile {}: {}", path, ex.toString());
            return null;
        }
    }

    private static ZoneAtmosphereProfileRegistry fallbackDefaults() {
        Map<String, ZoneAtmosphereProfile> defaults = new LinkedHashMap<>();
        defaults.put("spawn_plain", profile("spawn_plain", 0xB0C4DE, 0.15, 0xE8E8F0, "cloud256_dust", 0xD0D0D0, 0.5, "ambient_spawn_plain"));
        defaults.put("qingyun_peaks", profile("qingyun_peaks", 0x8090A0, 0.30, 0xC0C8D8, "cloud256_dust", 0xC0C8D0, 1.5, "ambient_qingyun_peaks"));
        defaults.put("blood_valley", profile("blood_valley", 0x5A2020, 0.35, 0x604040, "tribulation_spark", 0xFF4444, 2.3, "ambient_blood_valley"));
        defaults.put("spring_marsh", profile("spring_marsh", 0xA0C8A0, 0.25, 0xD0E8D0, "lingqi_ripple", 0x88CC88, 1.5, "ambient_spring_marsh"));
        defaults.put("north_wastes", profile("north_wastes", 0x909090, 0.50, 0xB0B0B0, "cloud256_dust", 0xA0A0A0, 3.0, "ambient_north_wastes"));
        defaults.put("wilderness", profile("wilderness", 0xC0C0B0, 0.10, 0xD8D8D0, "cloud256_dust", 0xB0B0A0, 0.3, "ambient_wilderness"));
        defaults.put("dark_cavern", profile("dark_cavern", 0x303038, 0.45, 0x383848, "cloud256_dust", 0x686878, 1.1, "ambient_dark_cavern"));
        defaults.put("tsy", profile("tsy", 0x404050, 0.30, 0x202030, "cloud256_dust", 0x707080, 1.0, "ambient_tsy"));
        return new ZoneAtmosphereProfileRegistry(defaults);
    }

    private static ZoneAtmosphereProfile profile(
        String zoneId,
        int fogColorRgb,
        double fogDensity,
        int skyTintRgb,
        String particleType,
        int particleTintRgb,
        double particleDensity,
        String ambientRecipeId
    ) {
        return new ZoneAtmosphereProfile(
            zoneId,
            fogColorRgb,
            fogDensity,
            List.of(new ZoneAtmosphereProfile.ParticleConfig(particleType, particleTintRgb, particleDensity, 0.01, 0.0, 0.0, 20)),
            skyTintRgb,
            ZoneAtmosphereProfile.TransitionFx.FADE,
            ambientRecipeId
        );
    }

    private static String normalizeZoneId(String zoneId) {
        String normalized = zoneId == null ? "" : zoneId.trim();
        return normalized.isEmpty() ? "wilderness" : normalized;
    }
}
