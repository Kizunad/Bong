package com.bong.client.fauna;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.Arrays;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class FaunaRenderBootstrapTest {
    @Test
    void faunaVisualKindsPinEntityRawIdOrderAfterWhale() {
        assertEquals(126, FaunaVisualKind.DEVOUR_RAT.expectedRawId());
        assertEquals(127, FaunaVisualKind.ASH_SPIDER.expectedRawId());
        assertEquals(128, FaunaVisualKind.HYBRID_BEAST.expectedRawId());
        assertEquals(129, FaunaVisualKind.VOID_DISTORTED.expectedRawId());
        assertEquals(130, FaunaVisualKind.DAOXIANG.expectedRawId());
        assertEquals(131, FaunaVisualKind.ZHINIAN.expectedRawId());
        assertEquals(132, FaunaVisualKind.TSY_SENTINEL.expectedRawId());
        assertEquals(133, FaunaVisualKind.FUYA.expectedRawId());
    }

    @Test
    void allPlannedNonWhaleFaunaModelsHaveStableResourcePaths() {
        Set<String> paths = Arrays.stream(FaunaVisualKind.values())
            .map(kind -> kind.modelId().getPath())
            .collect(Collectors.toSet());

        assertTrue(paths.contains("geo/devour_rat.geo.json"));
        assertTrue(paths.contains("geo/ash_spider.geo.json"));
        assertTrue(paths.contains("geo/hybrid_beast.geo.json"));
        assertTrue(paths.contains("geo/void_distorted.geo.json"));
        assertTrue(paths.contains("geo/daoxiang.geo.json"));
        assertTrue(paths.contains("geo/zhinian.geo.json"));
        assertTrue(paths.contains("geo/tsy_sentinel.geo.json"));
        assertTrue(paths.contains("geo/fuya.geo.json"));
    }

    @Test
    void fuyaTextureUsesEntityFaunaNamespace() {
        Identifier texture = FaunaVisualKind.FUYA.textureId();
        assertEquals("bong", texture.getNamespace());
        assertEquals("textures/entity/fauna/fuya.png", texture.getPath());
    }
}
