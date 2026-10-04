package com.bong.client.alchemy;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertThrows;

class AlchemyWorldEffectsTest {
    @Test
    void expiredWorldEffectsResetFurnaceModelBeforeDroppingRegistry() {
        var models = new LinkedHashMap<BlockPos, String>();
        models.put(new BlockPos(4, 70, -2), "furnace-model");
        var resets = new AtomicInteger();

        AlchemyWorldEffects.resetFurnaceModels(models, ignored -> resets.incrementAndGet());

        assertTrue(models.isEmpty(), "过期世界表现必须移除模型索引");
        assertEquals(1, resets.get(), "清理模型索引前必须重置已登记的炉体表现");
    }

    @Test
    void resetFurnaceModelsContinuesAfterOneModelFailsAndAggregatesFailures() {
        var models = new LinkedHashMap<BlockPos, String>();
        models.put(new BlockPos(1, 70, 1), "first");
        models.put(new BlockPos(2, 70, 2), "second");
        models.put(new BlockPos(3, 70, 3), "third");
        var visited = new java.util.ArrayList<String>();
        var first = new IllegalStateException("first reset failed");
        var later = new IllegalArgumentException("later reset failed");

        var thrown = assertThrows(IllegalStateException.class, () ->
            AlchemyWorldEffects.resetFurnaceModels(models, model -> {
                visited.add(model);
                if (model.equals("first")) throw first;
                if (model.equals("third")) throw later;
            }));

        assertEquals(3, visited.size(), "一个炉体重置失败不能跳过其他炉体");
        assertTrue(visited.containsAll(List.of("first", "second", "third")));
        assertSame(first, thrown);
        assertEquals(List.of(later), List.of(thrown.getSuppressed()));
        assertTrue(models.isEmpty(), "所有重置尝试完成后必须清空模型索引");
    }

    @Test
    void resetFurnaceModelsResetsRemainingModelsAfterFirstFailure() {
        var models = new LinkedHashMap<BlockPos, String>();
        models.put(new BlockPos(4, 70, 4), "first");
        models.put(new BlockPos(5, 70, 5), "second");
        models.put(new BlockPos(6, 70, 6), "third");
        var reset = new java.util.ArrayList<String>();

        assertThrows(IllegalStateException.class, () ->
            AlchemyWorldEffects.resetFurnaceModels(models, model -> {
                if (model.equals("first")) throw new IllegalStateException("first reset failed");
                reset.add(model);
            }));

        assertEquals(List.of("second", "third"), reset,
            "首个模型失败后，剩余模型仍必须完成重置");
        assertTrue(models.isEmpty(), "所有重置尝试完成后必须清空模型索引");
    }
}
