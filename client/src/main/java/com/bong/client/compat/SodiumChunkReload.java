package com.bong.client.compat;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.world.chunk.WorldChunk;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.lang.reflect.Field;
import java.lang.reflect.Method;

/**
 * Valence sends chunks incrementally (1/tick) and ramps view distance over
 * several seconds.  Sodium 0.5.x tracks chunks via mixin hooks on
 * loadChunkFromPacket / readLightData, but a race between Valence's view-
 * distance ramp and its chunk-layer messaging causes some chunks to arrive
 * without triggering Sodium's hooks.  Those chunks exist in the vanilla
 * ClientChunkManager but are invisible to Sodium's ChunkTracker.
 *
 * This listener runs once per second after world join and re-fires
 * onChunkStatusAdded for any chunk the vanilla manager has but Sodium's
 * tracker doesn't.  Once all chunks are synced it stops polling.
 */
public class SodiumChunkReload {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-client");
    private static boolean sodiumPresent;
    private static int ticksSinceLastSync = 0;
    private static boolean fullySynced = false;

    public static void register() {
        try {
            Class.forName("me.jellysquid.mods.sodium.client.render.chunk.map.ChunkTrackerHolder");
            sodiumPresent = true;
        } catch (ClassNotFoundException e) {
            sodiumPresent = false;
            return;
        }

        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            if (!sodiumPresent || fullySynced) return;
            if (client.world == null) return;
            ticksSinceLastSync++;
            if (ticksSinceLastSync < 20) return;
            ticksSinceLastSync = 0;
            resyncMissingChunks(client);
        });
    }

    private static void resyncMissingChunks(MinecraftClient client) {
        ClientWorld world = client.world;
        if (world == null) return;

        try {
            Class<?> holderClass = Class.forName(
                "me.jellysquid.mods.sodium.client.render.chunk.map.ChunkTrackerHolder");
            Method getMethod = holderClass.getMethod("get", ClientWorld.class);
            Object tracker = getMethod.invoke(null, world);

            Field statusField = tracker.getClass().getDeclaredField("chunkStatus");
            statusField.setAccessible(true);
            var statusMap = (it.unimi.dsi.fastutil.longs.Long2IntOpenHashMap) statusField.get(tracker);

            Method addStatus = tracker.getClass().getMethod(
                "onChunkStatusAdded", int.class, int.class, int.class);

            var chunkManager = world.getChunkManager();
            int synced = 0;

            for (int cx = -128; cx < 128; cx++) {
                for (int cz = -128; cz < 128; cz++) {
                    WorldChunk chunk = chunkManager.getWorldChunk(cx, cz, false);
                    if (chunk == null) continue;
                    long key = net.minecraft.util.math.ChunkPos.toLong(cx, cz);
                    int status = statusMap.getOrDefault(key, 0);
                    if (status == 3) continue;
                    addStatus.invoke(tracker, cx, cz, 3);
                    synced++;
                }
            }

            if (synced > 0) {
                LOGGER.info("[SodiumCompat] Re-synced {} chunks to Sodium tracker", synced);
            } else {
                fullySynced = true;
                LOGGER.info("[SodiumCompat] All chunks in sync with Sodium tracker");
            }
        } catch (Exception e) {
            LOGGER.warn("[SodiumCompat] Failed to resync chunks: {}", e.getMessage());
        }
    }
}
