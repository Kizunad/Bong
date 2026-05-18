package com.bong.client.dandao;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dandao-path-v1 P3 -- Tests for MutationInspectLabel.
 * Validates inspect visibility rules per §4.3.
 */
class MutationInspectLabelTest {
    @Test
    void stageZeroNotVisibleOnInspect() {
        assertFalse(MutationInspectLabel.isVisibleOnInspect(0));
        assertTrue(MutationInspectLabel.buildLabels(0, List.of()).isEmpty());
    }

    @Test
    void stageOneNotVisibleOnInspect() {
        assertFalse(MutationInspectLabel.isVisibleOnInspect(1));
        assertTrue(MutationInspectLabel.buildLabels(1, List.of()).isEmpty());
    }

    @Test
    void stageTwoShowsVisibleTag() {
        assertTrue(MutationInspectLabel.isVisibleOnInspect(2));

        List<String> labels = MutationInspectLabel.buildLabels(2, List.of(
            new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1)
        ));

        assertFalse(labels.isEmpty());
        assertTrue(labels.stream().anyMatch(l -> l.contains("显变")),
            "Stage 2 should show '显变' tag");
        assertTrue(labels.stream().anyMatch(l -> l.contains("金瞳")),
            "Should translate GoldenIris to 金瞳");
    }

    @Test
    void stageThreeShowsHeavyTag() {
        List<String> labels = MutationInspectLabel.buildLabels(3, List.of(
            new MutationVisualState.MutationSlotEntry("Horns", "Head", 2)
        ));

        assertTrue(labels.stream().anyMatch(l -> l.contains("重变")),
            "Stage 3 should show '重变' tag");
    }

    @Test
    void stageFourShowsBeastLabel() {
        List<String> labels = MutationInspectLabel.buildLabels(4, List.of(
            new MutationVisualState.MutationSlotEntry("ExtraArms", "Torso", 3),
            new MutationVisualState.MutationSlotEntry("BeastFace", "Head", 2)
        ));

        assertTrue(labels.get(0).contains("兽化体"),
            "Stage 4 should show '兽化体' as first label (red bold)");
        assertTrue(labels.get(0).contains("§c§l"),
            "Stage 4 兽化体 label should be red bold (§c§l)");
        assertTrue(labels.stream().anyMatch(l -> l.contains("兽化")),
            "Stage 4 should show '兽化' tag");
        assertTrue(labels.stream().anyMatch(l -> l.contains("多臂")),
            "Should translate ExtraArms to 多臂");
        assertTrue(labels.stream().anyMatch(l -> l.contains("兽面")),
            "Should translate BeastFace to 兽面");
    }

    @Test
    void allMutationKindsTranslated() {
        assertEquals("金瞳", MutationInspectLabel.translateKind("GoldenIris"));
        assertEquals("硬甲指", MutationInspectLabel.translateKind("HardenedNails"));
        assertEquals("糙皮", MutationInspectLabel.translateKind("ToughSkin"));
        assertEquals("额骨脊", MutationInspectLabel.translateKind("BoneRidge"));
        assertEquals("前臂鳞", MutationInspectLabel.translateKind("ForearmScales"));
        assertEquals("脊突", MutationInspectLabel.translateKind("SpineSpurs"));
        assertEquals("双角", MutationInspectLabel.translateKind("Horns"));
        assertEquals("尾", MutationInspectLabel.translateKind("Tail"));
        assertEquals("背甲", MutationInspectLabel.translateKind("BackCarapace"));
        assertEquals("多臂", MutationInspectLabel.translateKind("ExtraArms"));
        assertEquals("体型膨胀", MutationInspectLabel.translateKind("BodyEnlarge"));
        assertEquals("兽面", MutationInspectLabel.translateKind("BeastFace"));
    }

    @Test
    void unknownKindPassedThrough() {
        assertEquals("SomeUnknown", MutationInspectLabel.translateKind("SomeUnknown"));
    }

    @Test
    void nullKindReturnsDefault() {
        assertEquals("未知", MutationInspectLabel.translateKind(null));
    }

    @Test
    void allBodySlotsTranslated() {
        assertEquals("头部", MutationInspectLabel.translateBodySlot("Head"));
        assertEquals("前臂", MutationInspectLabel.translateBodySlot("Forearm"));
        assertEquals("背部", MutationInspectLabel.translateBodySlot("Back"));
        assertEquals("躯干", MutationInspectLabel.translateBodySlot("Torso"));
        assertEquals("下肢", MutationInspectLabel.translateBodySlot("Lower"));
    }

    @Test
    void bodySlotTranslationCaseInsensitive() {
        assertEquals("头部", MutationInspectLabel.translateBodySlot("HEAD"));
        assertEquals("头部", MutationInspectLabel.translateBodySlot("head"));
    }

    @Test
    void slotLevelShownInLabel() {
        List<String> labels = MutationInspectLabel.buildLabels(2, List.of(
            new MutationVisualState.MutationSlotEntry("ForearmScales", "Forearm", 2)
        ));

        assertTrue(labels.stream().anyMatch(l -> l.contains("Lv.2")),
            "Label should include slot level");
        assertTrue(labels.stream().anyMatch(l -> l.contains("[前臂]")),
            "Label should include translated body slot");
    }
}
