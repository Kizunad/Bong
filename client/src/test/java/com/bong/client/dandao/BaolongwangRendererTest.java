package com.bong.client.dandao;

import org.junit.jupiter.api.Test;

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

}
