package com.bong.client.dandao;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dandao-path-v1 P3 -- Tests for MutationKind enum.
 * Pin test: every variant resolves from server CamelCase name.
 */
class MutationKindTest {
    @Test
    void allVariantsResolveFromServerName() {
        // Stage 1
        assertEquals(MutationKind.GOLDEN_IRIS, MutationKind.fromServerName("GoldenIris"));
        assertEquals(MutationKind.HARDENED_NAILS, MutationKind.fromServerName("HardenedNails"));
        assertEquals(MutationKind.TOUGH_SKIN, MutationKind.fromServerName("ToughSkin"));
        // Stage 2
        assertEquals(MutationKind.BONE_RIDGE, MutationKind.fromServerName("BoneRidge"));
        assertEquals(MutationKind.FOREARM_SCALES, MutationKind.fromServerName("ForearmScales"));
        assertEquals(MutationKind.SPINE_SPURS, MutationKind.fromServerName("SpineSpurs"));
        // Stage 3
        assertEquals(MutationKind.HORNS, MutationKind.fromServerName("Horns"));
        assertEquals(MutationKind.TAIL, MutationKind.fromServerName("Tail"));
        assertEquals(MutationKind.BACK_CARAPACE, MutationKind.fromServerName("BackCarapace"));
        // Stage 4
        assertEquals(MutationKind.EXTRA_ARMS, MutationKind.fromServerName("ExtraArms"));
        assertEquals(MutationKind.BODY_ENLARGE, MutationKind.fromServerName("BodyEnlarge"));
        assertEquals(MutationKind.BEAST_FACE, MutationKind.fromServerName("BeastFace"));
    }

    @Test
    void unknownServerNameReturnsNull() {
        assertNull(MutationKind.fromServerName("SomeUnknownMutation"));
    }

    @Test
    void nullServerNameReturnsNull() {
        assertNull(MutationKind.fromServerName(null));
    }

    @Test
    void blankServerNameReturnsNull() {
        assertNull(MutationKind.fromServerName(""));
        assertNull(MutationKind.fromServerName("   "));
    }

    @Test
    void geoIdFormat() {
        assertEquals("bong:geo/dandao_golden_iris.geo.json", MutationKind.GOLDEN_IRIS.geoId().toString());
        assertEquals("bong:geo/dandao_horns.geo.json", MutationKind.HORNS.geoId().toString());
    }

    @Test
    void textureIdFormat() {
        assertEquals("bong:textures/entity/mutation/dandao_iris.png", MutationKind.GOLDEN_IRIS.textureId().toString());
        assertEquals("bong:textures/entity/mutation/dandao_horns.png", MutationKind.HORNS.textureId().toString());
    }

    @Test
    void defaultBodySlotAssigned() {
        assertEquals("Head", MutationKind.GOLDEN_IRIS.defaultBodySlot());
        assertEquals("Forearm", MutationKind.HARDENED_NAILS.defaultBodySlot());
        assertEquals("Back", MutationKind.SPINE_SPURS.defaultBodySlot());
        assertEquals("Torso", MutationKind.EXTRA_ARMS.defaultBodySlot());
        assertEquals("Lower", MutationKind.TAIL.defaultBodySlot());
    }

    @Test
    void enumCount() {
        assertEquals(12, MutationKind.values().length,
            "Should have 12 mutation kinds (3 per stage x 4 stages)");
    }
}
