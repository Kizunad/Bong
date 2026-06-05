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
        // raw_id 146-159: contiguous block (plan-entity-model-v1 / plan-supply-coffin-v1).
        // raw_id 160-163: plan-coffin-tiers-v1 延寿棺四档，与上面同在一个注册循环里连号。
        // Baolongwang BOSS（独立 bootstrap）在本枚举之后注册，取 164 —— 不在本枚举内，
        // 故本枚举的 raw_id 必须是从 146 起一路连续无空位（早期"160 留给 boss"的设计
        // 已废弃：单循环注册无法在中间插空）。
        int expectedRawId = 146;
        for (BongEntityModelKind kind : BongEntityRegistry.orderedKindsForTests()) {
            assertEquals(
                expectedRawId++,
                kind.expectedRawId(),
                "Entity model raw ids must start after fauna raw_id=145 and stay strictly sequential (no gaps)"
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

        assertEquals(145, maxFaunaRawId, "Entity model ids must move if fauna reserves more ids");
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

    /**
     * plan-coffin-tiers-v1 P3 — raw_id pin tests (client ↔ server cross-repo contract).
     * Must stay 1:1 with server/src/world/entity_model.rs COFFIN_*_ENTITY_KIND constants.
     */
    @Test
    void coffinTiersRawIdsPinMatchServerConstants() {
        assertEquals(160, BongEntityModelKind.COFFIN_MUNDANE.expectedRawId(),
            "COFFIN_MUNDANE raw_id must match server COFFIN_MUNDANE_ENTITY_KIND=160");
        assertEquals(161, BongEntityModelKind.COFFIN_JADE.expectedRawId(),
            "COFFIN_JADE raw_id must match server COFFIN_JADE_ENTITY_KIND=161");
        assertEquals(162, BongEntityModelKind.COFFIN_STONE.expectedRawId(),
            "COFFIN_STONE raw_id must match server COFFIN_STONE_ENTITY_KIND=162");
        assertEquals(163, BongEntityModelKind.COFFIN_BRONZE.expectedRawId(),
            "COFFIN_BRONZE raw_id must match server COFFIN_BRONZE_ENTITY_KIND=163");
    }

    @Test
    void coffinTiersRenderersRegister() {
        assertRenderer(BongEntityModelKind.COFFIN_MUNDANE, CoffinMundaneRenderer.class, 1);
        assertRenderer(BongEntityModelKind.COFFIN_JADE, CoffinJadeRenderer.class, 1);
        assertRenderer(BongEntityModelKind.COFFIN_STONE, CoffinStoneRenderer.class, 1);
        assertRenderer(BongEntityModelKind.COFFIN_BRONZE, CoffinBronzeRenderer.class, 1);
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
