package com.bong.client.skill;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertTrue;

class SkillSetSnapshotTest {

    @Test
    void consumedScrollsArePreservedEvenWhenSkillsMapIsEmpty() {
        SkillSetSnapshot snapshot = SkillSetSnapshot.of(
            java.util.Map.of(),
            java.util.Set.of("skill_scroll_herbalism_baicao_can")
        );

        assertTrue(snapshot.hasConsumedScroll("skill_scroll_herbalism_baicao_can"));
    }
}
