package com.bong.client.alchemy;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import java.util.HashMap;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AlchemyWorldEffectsTest {
    @Test
    void expiredWorldEffectsResetFurnaceModelBeforeDroppingRegistry() {
        var models = new HashMap<BlockPos, String>();
        models.put(new BlockPos(4, 70, -2), "furnace-model");
        var resets = new AtomicInteger();

        AlchemyWorldEffects.resetFurnaceModels(models, ignored -> resets.incrementAndGet());

        assertTrue(models.isEmpty(), "过期世界表现必须移除模型索引");
        assertEquals(1, resets.get(), "清理模型索引前必须重置已登记的炉体表现");
    }
}
