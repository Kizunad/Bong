package com.bong.client.entity;

import com.bong.client.fauna.FaunaVisualKind;
import com.bong.client.whale.WhaleEntities;
import org.junit.jupiter.api.Test;

import java.util.EnumMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongEntityModelRegistryTest {
    @Test
    void allPlanEntitiesHaveRendererBindings() {
        Map<BongEntityModelKind, Class<? extends BongModeledEntityRenderer>> bindings =
            BongEntityRenderBootstrap.rendererBindingsForTests();

        assertEquals(
            Set.of(BongEntityModelKind.values()),
            bindings.keySet(),
            "Every plan-entity-model-v1 visual entity must bind a Fabric renderer"
        );
    }

    @Test
    void rawIdsStayAfterFaunaWithoutShiftingExistingContract() {
        int expectedRawId = 143;
        for (BongEntityModelKind kind : BongEntityRegistry.orderedKindsForTests()) {
            assertEquals(
                expectedRawId++,
                kind.expectedRawId(),
                "Entity model raw ids must start after fauna raw_id=142 and stay sequential"
            );
        }
    }

    @Test
    void entityModelRawIdsDoNotOverlapWhaleOrFaunaVisualShells() {
        Set<Integer> occupied = new HashSet<>();
        assertTrue(occupied.add(WhaleEntities.EXPECTED_RAW_ID), "whale raw id must be unique");
        int maxFaunaRawId = WhaleEntities.EXPECTED_RAW_ID;
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            assertTrue(occupied.add(kind.expectedRawId()), "Duplicate fauna raw id: " + kind);
            maxFaunaRawId = Math.max(maxFaunaRawId, kind.expectedRawId());
        }

        assertEquals(142, maxFaunaRawId, "Entity model ids must move if fauna reserves more ids");
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            assertTrue(
                kind.expectedRawId() > maxFaunaRawId,
                "Entity model raw id must stay after fauna range: " + kind
            );
            assertTrue(occupied.add(kind.expectedRawId()), "Duplicate entity model raw id: " + kind);
        }
    }

    @Test
    void rendererResourcesAreUniquePerEntityKind() {
        Set<String> modelResources = new HashSet<>();
        Set<String> animationResources = new HashSet<>();
        Set<String> textureResources = new HashSet<>();

        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            assertTrue(modelResources.add(kind.modelResource().toString()), "Duplicate model: " + kind);
            assertTrue(animationResources.add(kind.animationResource().toString()), "Duplicate animation: " + kind);
            for (int state = 0; state < kind.stateCount(); state++) {
                assertTrue(textureResources.add(kind.textureForState(state).toString()), "Duplicate texture: " + kind);
            }
        }
    }

    @Test
    void spiritNicheRenders() {
        assertRenderer(BongEntityModelKind.SPIRIT_NICHE, SpiritNicheRenderer.class, 3);
    }

    @Test
    void spiritEyeRenders() {
        assertRenderer(BongEntityModelKind.SPIRIT_EYE, SpiritEyeRenderer.class, 3);
    }

    @Test
    void riftPortalRenders() {
        assertRenderer(BongEntityModelKind.RIFT_PORTAL, RiftPortalRenderer.class, 3);
    }

    @Test
    void workbenchRenderersRegister() {
        assertRenderer(BongEntityModelKind.FORGE_STATION, ForgeStationRenderer.class, 2);
        assertRenderer(BongEntityModelKind.ALCHEMY_FURNACE, AlchemyFurnaceRenderer.class, 2);
        assertRenderer(BongEntityModelKind.FORMATION_CORE, FormationCoreRenderer.class, 3);
    }

    @Test
    void lingtianAndTsyContainerRenderersRegister() {
        assertRenderer(BongEntityModelKind.LINGTIAN_PLOT, LingtianPlotRenderer.class, 4);
        assertRenderer(BongEntityModelKind.DRY_CORPSE, DryCorpseRenderer.class, 3);
        assertRenderer(BongEntityModelKind.BONE_SKELETON, BoneSkeletonRenderer.class, 3);
        assertRenderer(BongEntityModelKind.STORAGE_POUCH, StoragePouchRenderer.class, 3);
        assertRenderer(BongEntityModelKind.STONE_CASKET, StoneCasketRenderer.class, 3);
    }

    @Test
    void expectedRawIdMapCoversAllKinds() {
        EnumMap<BongEntityModelKind, Integer> expected = new EnumMap<>(BongEntityModelKind.class);
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            expected.put(kind, kind.expectedRawId());
        }
        assertEquals(expected, BongEntityRegistry.expectedRawIdsForTests());
    }

    @Test
    void visualStateIsClampedInsteadOfWrapped() {
        assertEquals(0, BongEntityModelKind.SPIRIT_NICHE.normalizeVisualState(-1));
        assertEquals(0, BongEntityModelKind.SPIRIT_NICHE.normalizeVisualState(0));
        assertEquals(2, BongEntityModelKind.SPIRIT_NICHE.normalizeVisualState(99));
        assertEquals(
            "bong:textures/entity/spirit_niche_invaded.png",
            BongEntityModelKind.SPIRIT_NICHE.textureForState(99).toString()
        );
    }

    @Test
    void modelFallsBackToRendererKindWhenEntityIsNull() {
        BongModeledEntityModel model = new BongModeledEntityModel(BongEntityModelKind.RIFT_PORTAL);
        assertEquals(
            BongEntityModelKind.RIFT_PORTAL.textureForState(0),
            model.getTextureResource(null)
        );
        assertEquals(BongEntityModelKind.RIFT_PORTAL.modelResource(), model.getModelResource(null));
        assertEquals(BongEntityModelKind.RIFT_PORTAL.animationResource(), model.getAnimationResource(null));
    }

    private static void assertRenderer(
        BongEntityModelKind kind,
        Class<? extends BongModeledEntityRenderer> rendererClass,
        int stateCount
    ) {
        assertSame(rendererClass, BongEntityRenderBootstrap.rendererBindingsForTests().get(kind));
        assertEquals(stateCount, kind.stateCount(), "Unexpected visual state count for " + kind);
    }
}
