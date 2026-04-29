package com.bong.client.preview;

import org.junit.jupiter.api.Test;

import java.util.HashSet;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ChunkReadyCheckerTest {

    /** Helper: 用 Set<Long> 模拟"已加载 chunk 集合"，loadedKey = (chunkX << 32) | chunkZ。 */
    private static ChunkReadyChecker.ChunkLoadedQuery loadedSet(Set<Long> loaded) {
        return (cx, cz) -> loaded.contains(((long) cx << 32) | (cz & 0xFFFFFFFFL));
    }

    private static long key(int cx, int cz) {
        return ((long) cx << 32) | (cz & 0xFFFFFFFFL);
    }

    @Test
    void radius0SingleChunkLoaded() {
        Set<Long> loaded = new HashSet<>();
        loaded.add(key(0, 0));
        assertTrue(ChunkReadyChecker.allLoaded(0, 0, 0, loadedSet(loaded)));
    }

    @Test
    void radius0SingleChunkMissing() {
        Set<Long> loaded = new HashSet<>();  // 空 = 无 chunk 加载
        assertFalse(ChunkReadyChecker.allLoaded(0, 0, 0, loadedSet(loaded)));
    }

    @Test
    void radius1NineChunksAllLoaded() {
        Set<Long> loaded = new HashSet<>();
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                loaded.add(key(5 + dx, -3 + dz));
            }
        }
        assertTrue(ChunkReadyChecker.allLoaded(5, -3, 1, loadedSet(loaded)));
    }

    @Test
    void radius1OneCornerMissing() {
        Set<Long> loaded = new HashSet<>();
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                loaded.add(key(0 + dx, 0 + dz));
            }
        }
        loaded.remove(key(-1, -1));  // 去 NW 角
        assertFalse(ChunkReadyChecker.allLoaded(0, 0, 1, loadedSet(loaded)));
    }

    @Test
    void radius2QuarterMissing() {
        // 25 chunks，少中心也算缺
        Set<Long> loaded = new HashSet<>();
        for (int dx = -2; dx <= 2; dx++) {
            for (int dz = -2; dz <= 2; dz++) {
                loaded.add(key(dx, dz));
            }
        }
        loaded.remove(key(0, 0));
        assertFalse(ChunkReadyChecker.allLoaded(0, 0, 2, loadedSet(loaded)));
    }

    @Test
    void radius4FullCoverage() {
        Set<Long> loaded = new HashSet<>();
        for (int dx = -4; dx <= 4; dx++) {
            for (int dz = -4; dz <= 4; dz++) {
                loaded.add(key(100 + dx, 100 + dz));
            }
        }
        assertTrue(ChunkReadyChecker.allLoaded(100, 100, 4, loadedSet(loaded)));
        // 总数 9*9 = 81
        assertEquals(81, loaded.size());
    }

    @Test
    void negativeRadiusRejected() {
        assertThrows(IllegalArgumentException.class,
                () -> ChunkReadyChecker.allLoaded(0, 0, -1, (cx, cz) -> true));
    }

    @Test
    void allFalseQueryAlwaysFails() {
        // 防退化：query 全 false 时任何 radius 都应返回 false
        ChunkReadyChecker.ChunkLoadedQuery alwaysFalse = (cx, cz) -> false;
        assertFalse(ChunkReadyChecker.allLoaded(0, 0, 0, alwaysFalse));
        assertFalse(ChunkReadyChecker.allLoaded(0, 0, 1, alwaysFalse));
        assertFalse(ChunkReadyChecker.allLoaded(-50, 50, 4, alwaysFalse));
    }

    @Test
    void allTrueQueryAlwaysPasses() {
        ChunkReadyChecker.ChunkLoadedQuery alwaysTrue = (cx, cz) -> true;
        assertTrue(ChunkReadyChecker.allLoaded(0, 0, 0, alwaysTrue));
        assertTrue(ChunkReadyChecker.allLoaded(1234, -5678, 8, alwaysTrue));
    }

    @Test
    void blockToChunkPositive() {
        assertEquals(0, ChunkReadyChecker.blockToChunk(0));
        assertEquals(0, ChunkReadyChecker.blockToChunk(15.999));
        assertEquals(1, ChunkReadyChecker.blockToChunk(16));
        assertEquals(25, ChunkReadyChecker.blockToChunk(400));
    }

    @Test
    void blockToChunkNegativeFloorDiv() {
        // 关键：MC 用 floorDiv，负数往负无穷取整
        assertEquals(-1, ChunkReadyChecker.blockToChunk(-1));
        assertEquals(-1, ChunkReadyChecker.blockToChunk(-16));
        assertEquals(-2, ChunkReadyChecker.blockToChunk(-17));
        assertEquals(-25, ChunkReadyChecker.blockToChunk(-400));
    }

    @Test
    void blockToChunkFractional() {
        // tp 坐标可能是 0.5（玩家居中），floor 后 0 → cx=0
        assertEquals(0, ChunkReadyChecker.blockToChunk(0.5));
        assertEquals(0, ChunkReadyChecker.blockToChunk(15.5));
        // -0.5 floor → -1 → cx=-1
        assertEquals(-1, ChunkReadyChecker.blockToChunk(-0.5));
    }

    @Test
    void queryCallCountBoundedByArea() {
        // 验证不会 over-query —— radius=2 最多 25 次（5x5）
        int[] callCount = {0};
        ChunkReadyChecker.ChunkLoadedQuery counter = (cx, cz) -> {
            callCount[0]++;
            return true;
        };
        ChunkReadyChecker.allLoaded(0, 0, 2, counter);
        assertEquals(25, callCount[0]);
    }

    @Test
    void shortCircuitOnFirstMissing() {
        // 第一个未加载就返回 false，不继续 query 全部
        int[] callCount = {0};
        ChunkReadyChecker.ChunkLoadedQuery counter = (cx, cz) -> {
            callCount[0]++;
            return false;  // 第一个就 false
        };
        ChunkReadyChecker.allLoaded(0, 0, 4, counter);
        assertEquals(1, callCount[0], "expected short-circuit on first false");
    }
}
