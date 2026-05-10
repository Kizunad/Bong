package com.bong.client.visual.particle;

import com.bong.client.botany.BotanyPlantStageVisual;
import com.bong.client.botany.BotanyPlantStageVisualStore;
import com.bong.client.botany.PlantGrowthStage;
import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.Locale;
import java.util.Optional;

/** Server-driven stage heartbeat for wild botany plants. */
public final class BotanyPlantStagePlayer implements VfxPlayer {
    public static final Identifier ROUTE_ID = new Identifier("bong", "botany_plant_stage");
    private static final String PATH_PREFIX = "botany_plant_stage__";
    private static final int FALLBACK_RGB = 0x88CC88;
    private static final int DEFAULT_TTL_TICKS = 140;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;
        Optional<ParsedStageEvent> parsed = parseEventId(payload.eventId());
        if (parsed.isEmpty()) return;

        ParsedStageEvent event = parsed.get();
        long now = world.getTime();
        int ttl = payload.durationTicks().orElse(DEFAULT_TTL_TICKS);
        BotanyPlantStageVisualStore.upsert(new BotanyPlantStageVisual(
            keyFor(event.plantId(), payload.origin()),
            event.plantId(),
            event.stage(),
            payload.origin(),
            payload.colorRgb().orElse(FALLBACK_RGB),
            payload.strength().orElse(0.7),
            now + Math.max(1, ttl),
            now
        ));
    }

    public static boolean isStageEvent(Identifier id) {
        return parseEventId(id).isPresent();
    }

    public static Optional<ParsedStageEvent> parseEventId(Identifier id) {
        if (id == null || !"bong".equals(id.getNamespace())) {
            return Optional.empty();
        }
        String path = id.getPath();
        if (!path.startsWith(PATH_PREFIX)) {
            return Optional.empty();
        }
        String tail = path.substring(PATH_PREFIX.length());
        String[] parts = tail.split("__", 2);
        if (parts.length != 2 || parts[0].isBlank() || parts[1].isBlank()) {
            return Optional.empty();
        }
        String plantId = parts[0].trim().toLowerCase(Locale.ROOT);
        PlantGrowthStage stage = PlantGrowthStage.fromWireName(parts[1]);
        return Optional.of(new ParsedStageEvent(plantId, stage));
    }

    private static String keyFor(String plantId, double[] origin) {
        long x = Math.round(origin[0] * 16.0);
        long y = Math.round(origin[1] * 16.0);
        long z = Math.round(origin[2] * 16.0);
        return plantId + "@" + x + "," + y + "," + z;
    }

    public record ParsedStageEvent(String plantId, PlantGrowthStage stage) {
    }
}
