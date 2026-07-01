package com.bong.client.compat;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.ModContainer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.world.chunk.WorldChunk;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Optional;

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
// Compat: Sodium 0.5.x — ChunkTrackerHolder.get(), chunkStatus field, onChunkStatusAdded(cx, cz, 3).
// Sodium 0.6+ may rename/remove these; check on upgrade.
//
// F14：反射调用此前无版本守卫——失败只 LOGGER.warn 不带 sodium 版本号，出问题时无法直接从日志
// 定位是"哪个 Sodium 版本改了 API"。照 BongIrisCompat 先例（FabricLoader modContainer 取
// friendly version string，register() 时记一次日志），resyncMissingChunks 反射失败时也把当前
// sodiumVersion 带进警告；连续失败达阈值后关闭轮询（Sodium 非 gradle 依赖，无法编译期检测，
// 只能靠这层运行期熔断防止每 20 tick 无意义反射失败刷日志）。
public class SodiumChunkReload {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-client");
    // Sodium 0.5.x FLAG_HAS_BLOCK_DATA — chunk fully loaded with block data.
    private static final int SODIUM_FLAG_HAS_BLOCK_DATA = 3;
    /** 连续反射失败达此次数后禁用后续轮询（避免 API 改名后每 20 tick 无意义刷日志）。 */
    private static final int CONSECUTIVE_FAILURE_DISABLE_THRESHOLD = 5;

    private static boolean sodiumPresent;
    private static String sodiumVersion = "unknown";
    private static int ticksSinceLastSync = 0;
    private static boolean fullySynced = false;
    private static int consecutiveFailures = 0;

    public static void register() {
        try {
            Class.forName("me.jellysquid.mods.sodium.client.render.chunk.map.ChunkTrackerHolder");
            sodiumPresent = true;
        } catch (ClassNotFoundException e) {
            sodiumPresent = false;
            return;
        }

        sodiumVersion = resolveSodiumVersion();
        consecutiveFailures = 0;
        LOGGER.info("[SodiumCompat] Sodium detected v{}, chunk resync polling active", sodiumVersion);

        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            if (!sodiumPresent) return;
            if (client.world == null) {
                // World unloaded (disconnect / dimension change) — reset for next join.
                fullySynced = false;
                ticksSinceLastSync = 0;
                return;
            }
            if (fullySynced) return;
            ticksSinceLastSync++;
            if (ticksSinceLastSync < 20) return;
            ticksSinceLastSync = 0;
            resyncMissingChunks(client);
        });
    }

    /**
     * 查 Sodium mod 的 friendly version string（先例：{@link com.bong.client.iris.BongIrisCompat#init()}）。
     * Sodium 非 gradle 依赖（编译期不可见），只能运行期靠 {@link FabricLoader} 查 mod 元数据；
     * 取不到时 fallback "unknown"，不影响功能，只影响诊断日志的信息量。
     */
    static String resolveSodiumVersion() {
        try {
            Optional<ModContainer> container = FabricLoader.getInstance().getModContainer("sodium");
            return container
                .map(c -> c.getMetadata().getVersion().getFriendlyString())
                .orElse("unknown");
        } catch (RuntimeException e) {
            return "unknown";
        }
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
                    if (status == SODIUM_FLAG_HAS_BLOCK_DATA) continue;
                    addStatus.invoke(tracker, cx, cz, SODIUM_FLAG_HAS_BLOCK_DATA);
                    synced++;
                }
            }

            consecutiveFailures = 0;
            if (synced > 0) {
                LOGGER.info("[SodiumCompat] Re-synced {} chunks to Sodium tracker", synced);
            } else {
                fullySynced = true;
                LOGGER.info("[SodiumCompat] All chunks in sync with Sodium tracker");
            }
        } catch (Exception e) {
            consecutiveFailures++;
            LOGGER.warn(
                "[SodiumCompat] Failed to resync chunks (sodium v{}, attempt {}/{}): {}",
                sodiumVersion, consecutiveFailures, CONSECUTIVE_FAILURE_DISABLE_THRESHOLD, e.getMessage());
            if (consecutiveFailures >= CONSECUTIVE_FAILURE_DISABLE_THRESHOLD) {
                sodiumPresent = false;
                LOGGER.warn(
                    "[SodiumCompat] Disabling further resync polling after {} consecutive failures"
                        + " (sodium v{} reflection API likely changed — check ChunkTrackerHolder"
                        + " on upgrade)",
                    consecutiveFailures, sodiumVersion);
            }
        }
    }

    static boolean isSodiumPresentForTests() {
        return sodiumPresent;
    }

    static String sodiumVersionForTests() {
        return sodiumVersion;
    }

    static int consecutiveFailuresForTests() {
        return consecutiveFailures;
    }

    static int consecutiveFailureDisableThresholdForTests() {
        return CONSECUTIVE_FAILURE_DISABLE_THRESHOLD;
    }

    /** 测试专用：重置静态状态，防止跨测试污染（Sodium 非依赖，测试环境恒不可用）。 */
    static void resetForTests() {
        sodiumPresent = false;
        sodiumVersion = "unknown";
        ticksSinceLastSync = 0;
        fullySynced = false;
        consecutiveFailures = 0;
    }
}
