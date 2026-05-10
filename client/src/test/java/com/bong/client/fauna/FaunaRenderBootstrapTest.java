package com.bong.client.fauna;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.Arrays;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class FaunaRenderBootstrapTest {
    @Test
    void faunaVisualKindsPinEntityRawIdOrderAfterWhale() {
        assertEquals(134, FaunaVisualKind.DEVOUR_RAT.expectedRawId());
        assertEquals(135, FaunaVisualKind.ASH_SPIDER.expectedRawId());
        assertEquals(136, FaunaVisualKind.HYBRID_BEAST.expectedRawId());
        assertEquals(137, FaunaVisualKind.VOID_DISTORTED.expectedRawId());
        assertEquals(138, FaunaVisualKind.DAOXIANG.expectedRawId());
        assertEquals(139, FaunaVisualKind.ZHINIAN.expectedRawId());
        assertEquals(140, FaunaVisualKind.TSY_SENTINEL.expectedRawId());
        assertEquals(141, FaunaVisualKind.FUYA.expectedRawId());
    }

    @Test
    void allPlannedNonWhaleFaunaModelsHaveStableResourcePaths() {
        Set<String> paths = Arrays.stream(FaunaVisualKind.values())
            .map(kind -> kind.modelId().getPath())
            .collect(Collectors.toSet());

        Set<String> expected = Set.of(
            "geo/devour_rat.geo.json",
            "geo/ash_spider.geo.json",
            "geo/hybrid_beast.geo.json",
            "geo/void_distorted.geo.json",
            "geo/daoxiang.geo.json",
            "geo/zhinian.geo.json",
            "geo/tsy_sentinel.geo.json",
            "geo/fuya.geo.json"
        );
        assertEquals(expected, paths);
    }

    @Test
    void fuyaTextureUsesEntityFaunaNamespace() {
        Identifier texture = FaunaVisualKind.FUYA.textureId();
        assertEquals("bong", texture.getNamespace());
        assertEquals("textures/entity/fauna/fuya.png", texture.getPath());
    }
}
