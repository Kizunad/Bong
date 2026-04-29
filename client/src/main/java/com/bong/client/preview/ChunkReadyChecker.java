package com.bong.client.preview;

/**
 * Chunk-ready 判定（plan-worldgen-snapshot-v1 §4.2）。
 *
 * 用于 {@link PreviewSession} 在传送后等待目标点周围 N×N chunks 加载完成
 * 再拍照，避免 chunk 还在网络流式时就触发 ScreenshotRecorder 拍到纯 sky
 * clear color。
 *
 * 抽离成纯函数 + SAM 接口（{@link ChunkLoadedQuery}）让逻辑可独立单测
 * 而无需 MinecraftClient 启动 —— 实际调用在
 * {@code PreviewSession.stepSettle} 里把 query 实现成
 * {@code (cx, cz) -> client.world.getChunkManager().getWorldChunk(cx, cz) != null}。
 */
public final class ChunkReadyChecker {
    private ChunkReadyChecker() {}

    @FunctionalInterface
    public interface ChunkLoadedQuery {
        boolean isLoaded(int chunkX, int chunkZ);
    }

    /**
     * 判断 (centerChunkX, centerChunkZ) 周围 (2*radius+1) × (2*radius+1) 个 chunk
     * 是否全部加载。radius=0 表示只查中心 chunk；radius=4 = 9×9 = 81 chunks。
     *
     * @throws IllegalArgumentException radius < 0
     */
    public static boolean allLoaded(
            int centerChunkX,
            int centerChunkZ,
            int radius,
            ChunkLoadedQuery query) {
        if (radius < 0) {
            throw new IllegalArgumentException(
                    "ChunkReadyChecker.allLoaded radius must be >= 0, got " + radius);
        }
        for (int dz = -radius; dz <= radius; dz++) {
            for (int dx = -radius; dx <= radius; dx++) {
                if (!query.isLoaded(centerChunkX + dx, centerChunkZ + dz)) {
                    return false;
                }
            }
        }
        return true;
    }

    /**
     * Block 坐标 → chunk 坐标。MC chunks 16 blocks 宽，按 floorDiv（负数往负无穷取整）。
     *
     * <pre>
     * blockX=0   → cx=0
     * blockX=15  → cx=0
     * blockX=16  → cx=1
     * blockX=-1  → cx=-1
     * blockX=-16 → cx=-1
     * blockX=-17 → cx=-2
     * </pre>
     */
    public static int blockToChunk(double blockCoord) {
        return Math.floorDiv((int) Math.floor(blockCoord), 16);
    }
}
