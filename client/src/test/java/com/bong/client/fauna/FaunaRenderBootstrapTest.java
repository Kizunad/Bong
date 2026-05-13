package com.bong.client.fauna;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;

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
        assertEquals(134, FaunaVisualKind.SKULL_FIEND.expectedRawId());
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
            "geo/fuya.geo.json",
            "geo/skull_fiend.geo.json"
        );
        assertEquals(expected, paths);
    }

    @Test
    void fuyaTextureUsesEntityFaunaNamespace() {
        Identifier texture = FaunaVisualKind.FUYA.textureId();
        assertEquals("bong", texture.getNamespace());
        assertEquals("textures/entity/fauna/fuya.png", texture.getPath());
    }

    @Test
    void skullFiendTextureUsesEntityFaunaNamespace() {
        Identifier texture = FaunaVisualKind.SKULL_FIEND.textureId();
        assertEquals("bong", texture.getNamespace());
        assertEquals("textures/entity/fauna/skull_fiend.png", texture.getPath());
    }

    @Test
    void faunaEntityExplicitlyParticipatesInCrosshairPicking() {
        Method canHit = assertCanHitMethod();

        assertEquals(
            boolean.class,
            canHit.getReturnType(),
            "expected FaunaEntity.canHit to return boolean because crosshair picking reads this contract, actual: "
                + canHit.getReturnType()
        );
        assertEquals(
            FaunaEntity.class,
            canHit.getDeclaringClass(),
            "expected FaunaEntity to override canHit directly because the base Entity default is not hittable enough for fauna picking"
        );
    }

    private static Method assertCanHitMethod() {
        try {
            return FaunaEntity.class.getDeclaredMethod("canHit");
        } catch (NoSuchMethodException error) {
            throw new AssertionError(
                "expected FaunaEntity.canHit override so fauna can participate in crosshair picking",
                error
            );
        }
    }
}
