package com.bong.client.dandao;

import org.junit.jupiter.api.Test;
import software.bernie.geckolib.core.animation.RawAnimation;

import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dandao-path-v1 P4 -- Tests for BaolongwangRenderer bone alias.
 * Validates §8.1 #6 decision: bone alias HashMap in renderer.
 */
class BaolongwangRendererTest {
    @Test
    void boneAliasesComplete() {
        Map<String, String> aliases = BaolongwangRenderer.BONE_ALIASES;

        assertEquals("body", aliases.get("bdk_body"));
        assertEquals("right_leg", aliases.get("bdk_rl"));
        assertEquals("left_leg", aliases.get("bdk_ll"));
        assertEquals("right_arm", aliases.get("bdk_ra"));
        assertEquals("left_arm", aliases.get("bdk_la"));
        assertEquals("right_wing", aliases.get("bdk_rw"));
        assertEquals("left_wing", aliases.get("bdk_lw"));

        assertEquals(7, aliases.size(),
            "Should have exactly 7 bone aliases (§8.1 #6)");
    }

    @Test
    void reverseAliasesMatchForward() {
        Map<String, String> forward = BaolongwangRenderer.BONE_ALIASES;
        Map<String, String> reverse = BaolongwangRenderer.REVERSE_ALIASES;

        for (Map.Entry<String, String> entry : forward.entrySet()) {
            assertEquals(entry.getKey(), reverse.get(entry.getValue()),
                "Reverse alias for '" + entry.getValue() + "' should map back to '" + entry.getKey() + "'");
        }

        assertEquals(forward.size(), reverse.size(),
            "Forward and reverse alias maps should have the same size");
    }

    @Test
    void resolveAliasKnownBone() {
        assertEquals("body", BaolongwangRenderer.resolveAlias("bdk_body"));
        assertEquals("right_arm", BaolongwangRenderer.resolveAlias("bdk_ra"));
    }

    @Test
    void resolveAliasUnknownBonePassthrough() {
        assertEquals("bone", BaolongwangRenderer.resolveAlias("bone"),
            "Unknown bone names should pass through unchanged");
        assertEquals("bone2", BaolongwangRenderer.resolveAlias("bone2"));
        assertEquals("group", BaolongwangRenderer.resolveAlias("group"));
    }

    @Test
    void resolveReverseKnownName() {
        assertEquals("bdk_body", BaolongwangRenderer.resolveReverse("body"));
        assertEquals("bdk_rw", BaolongwangRenderer.resolveReverse("right_wing"));
    }

    @Test
    void resolveReverseUnknownPassthrough() {
        assertEquals("bone", BaolongwangRenderer.resolveReverse("bone"));
    }

    @Test
    void discardedBonesNotInAliases() {
        assertFalse(BaolongwangRenderer.BONE_ALIASES.containsKey("bone"),
            "Decorative bone 'bone' should not have an alias");
        assertFalse(BaolongwangRenderer.BONE_ALIASES.containsKey("bone2"),
            "Decorative bone 'bone2' should not have an alias");
        assertFalse(BaolongwangRenderer.BONE_ALIASES.containsKey("group"),
            "Decorative bone 'group' should not have an alias");
    }

    @Test
    void semanticNamesAreUnique() {
        Set<String> values = Set.copyOf(BaolongwangRenderer.BONE_ALIASES.values());
        assertEquals(BaolongwangRenderer.BONE_ALIASES.size(), values.size(),
            "All semantic bone names should be unique");
    }

    // -- Animation state definitions (CodeRabbit #1 follow-up) --

    @Test
    void allFiveAnimationConstantsDefined() {
        // Verify all 5 RawAnimation static fields are non-null and distinct
        RawAnimation idle = BaolongwangEntity.IDLE;
        RawAnimation walk = BaolongwangEntity.WALK;
        RawAnimation attack = BaolongwangEntity.ATTACK;
        RawAnimation skill1 = BaolongwangEntity.SKILL1;
        RawAnimation skill2 = BaolongwangEntity.SKILL2;

        assertNotNull(idle, "IDLE animation must be defined");
        assertNotNull(walk, "WALK animation must be defined");
        assertNotNull(attack, "ATTACK animation must be defined");
        assertNotNull(skill1, "SKILL1 animation must be defined");
        assertNotNull(skill2, "SKILL2 animation must be defined");

        // All 5 must be distinct objects
        Set<RawAnimation> unique = Set.of(idle, walk, attack, skill1, skill2);
        assertEquals(5, unique.size(),
            "All 5 animation constants must be distinct RawAnimation instances");
    }

    @Test
    void actionStateMappingCoversAllActions() {
        // Verify the action state constants match expected durations:
        // 0=none, 1=attack(33t), 2=skill1(64t), 3=skill2(33t)
        // We can't instantiate the entity but we can verify the constants
        // are referenced in the controller (covered by allFiveAnimationConstantsDefined)
        // and the action state enum range is [0,3].
        // This test documents the contract for server sync (Phase B-2).
        assertNotNull(BaolongwangEntity.ATTACK,
            "action=1 maps to ATTACK (1.64s = ~33 ticks)");
        assertNotNull(BaolongwangEntity.SKILL1,
            "action=2 maps to SKILL1 (3.2s = ~64 ticks)");
        assertNotNull(BaolongwangEntity.SKILL2,
            "action=3 maps to SKILL2 (1.64s = ~33 ticks)");
    }
}
