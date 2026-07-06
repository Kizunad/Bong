package com.bong.client.tsy;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.ReservedInteractionIntents;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TsyContainerSearchIntentHandlerTest {
    @AfterEach
    void tearDown() {
        TsyContainerStateStore.resetForTests();
    }

    @Test
    void candidateUsesCrosshairVisualContainerInsteadOfNearestContainer() {
        TsyContainerStateStore.upsert(view(42L, 1001, 2.0, 0.0, 0.0));
        TsyContainerStateStore.upsert(view(77L, 1002, 0.5, 0.0, 0.0));

        Optional<InteractCandidate> candidate =
            TsyContainerSearchIntentHandler.candidateForVisualHit(1001, 0.0, 0.0, 0.0);

        assertTrue(candidate.isPresent(), "准星命中 visual=1001 的可搜刮容器时应产出 SearchContainer candidate");
        assertEquals(
            "tsy_container:42",
            candidate.orElseThrow().debugLabel(),
            "即使 entity=77 更近，也必须选择准星命中的 entity=42，不能按 nearest 截胡"
        );
        assertEquals(4.0, candidate.orElseThrow().distanceSq());
        assertEquals(ReservedInteractionIntents.SEARCH_CONTAINER_PRIORITY, candidate.orElseThrow().priority());
    }

    @Test
    void nonContainerCrosshairDoesNotFallbackToNearbyNearestContainer() {
        TsyContainerStateStore.upsert(view(42L, 1001, 0.5, 0.0, 0.0));

        Optional<InteractCandidate> candidate =
            TsyContainerSearchIntentHandler.candidateForVisualHit(9999, 0.0, 0.0, 0.0);

        assertFalse(
            candidate.isPresent(),
            "准星 visual 不属于任何 TSY 容器时必须无候选；脚边最近容器不能截胡 NPC/玩家/开箱交互"
        );
    }

    @Test
    void dispatchUsesCandidateEntityIdAndRejectsChangedCrosshair() {
        TsyContainerStateStore.upsert(view(42L, 1001, 2.0, 0.0, 0.0));
        TsyContainerStateStore.upsert(view(77L, 1002, 0.5, 0.0, 0.0));
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.SearchContainer,
            ReservedInteractionIntents.SEARCH_CONTAINER_PRIORITY,
            4.0,
            "tsy_container:42"
        );

        assertEquals(
            42L,
            TsyContainerSearchIntentHandler.dispatchEntityIdForVisualHit(candidate, 1001, 0.0, 0.0, 0.0),
            "dispatch 必须发送 candidate 记录的 gameplay entity bits，而不是重新取最近 entity=77"
        );
        assertNull(
            TsyContainerSearchIntentHandler.dispatchEntityIdForVisualHit(candidate, 1002, 0.0, 0.0, 0.0),
            "candidate 指向 entity=42 但当前准星改到 visual=1002/entity=77 时应拒绝，不能改发到另一个容器"
        );
    }

    @Test
    void crosshairContainerWithinFiveBlocksCreatesCandidate() {
        TsyContainerStateStore.upsert(view(42L, 1001, 4.75, 0.0, 0.0));

        Optional<InteractCandidate> candidate =
            TsyContainerSearchIntentHandler.candidateForVisualHit(1001, 0.0, 0.0, 0.0);

        assertTrue(
            candidate.isPresent(),
            "准星命中 4-5 格内的 TSY 容器应产出 SearchContainer candidate"
        );
        assertEquals("tsy_container:42", candidate.orElseThrow().debugLabel());
    }

    @Test
    void outOfServerSearchRangeDoesNotCreateCandidate() {
        TsyContainerStateStore.upsert(view(42L, 1001, 5.01, 0.0, 0.0));

        Optional<InteractCandidate> candidate =
            TsyContainerSearchIntentHandler.candidateForVisualHit(1001, 0.0, 0.0, 0.0);

        assertFalse(
            candidate.isPresent(),
            "server SEARCH_INTERACT_RANGE_M 当前为 5.0；超过 5 格即使准星命中也不应发 start_search"
        );
    }

    @Test
    void depletedOccupiedOrMissingVisualMappingAreRejected() {
        TsyContainerStateStore.upsert(new TsyContainerView(
            1L, "dry_corpse", "tsy", 1.0, 0.0, 0.0, null, true, null, 1001
        ));
        TsyContainerStateStore.upsert(new TsyContainerView(
            2L, "dry_corpse", "tsy", 1.0, 0.0, 0.0, null, false, "other-player", 1002
        ));
        TsyContainerStateStore.upsert(new TsyContainerView(
            3L, "dry_corpse", "tsy", 1.0, 0.0, 0.0, null, false, null, null
        ));

        assertFalse(TsyContainerSearchIntentHandler.candidateForVisualHit(1001, 0.0, 0.0, 0.0).isPresent());
        assertFalse(TsyContainerSearchIntentHandler.candidateForVisualHit(1002, 0.0, 0.0, 0.0).isPresent());
        assertFalse(
            TsyContainerSearchIntentHandler.candidateForVisualHit(3, 0.0, 0.0, 0.0).isPresent(),
            "没有 visual_entity_id 映射的旧 payload 不能退回 entity_id/nearest 猜测，否则会复发截胡"
        );
    }

    @Test
    void candidateEntityIdParsingIsStrict() {
        assertEquals(
            42L,
            TsyContainerSearchIntentHandler.candidateEntityId(InteractCandidate.of(
                InteractIntent.SearchContainer, 100, 1.0, "tsy_container:42"
            ))
        );
        assertNull(TsyContainerSearchIntentHandler.candidateEntityId(null));
        assertNull(TsyContainerSearchIntentHandler.candidateEntityId(InteractCandidate.of(
            InteractIntent.SearchContainer, 100, 1.0, "talk_npc:42"
        )));
        assertNull(TsyContainerSearchIntentHandler.candidateEntityId(InteractCandidate.of(
            InteractIntent.SearchContainer, 100, 1.0, "tsy_container:not-a-number"
        )));
    }

    @Test
    void visualKindGateOnlyAllowsTsyContainerVisuals() {
        assertTrue(TsyContainerSearchIntentHandler.isTsyContainerVisualKind(BongEntityModelKind.DRY_CORPSE));
        assertTrue(TsyContainerSearchIntentHandler.isTsyContainerVisualKind(BongEntityModelKind.BONE_SKELETON));
        assertTrue(TsyContainerSearchIntentHandler.isTsyContainerVisualKind(BongEntityModelKind.STORAGE_POUCH));
        assertTrue(TsyContainerSearchIntentHandler.isTsyContainerVisualKind(BongEntityModelKind.STONE_CASKET));
        assertFalse(TsyContainerSearchIntentHandler.isTsyContainerVisualKind(BongEntityModelKind.COFFIN_COMMON));
        assertFalse(TsyContainerSearchIntentHandler.isTsyContainerVisualKind(BongEntityModelKind.SPIRIT_NICHE));
        assertFalse(TsyContainerSearchIntentHandler.isTsyContainerVisualKind(null));
    }

    private static TsyContainerView view(long entityId, int visualEntityId, double x, double y, double z) {
        return new TsyContainerView(
            entityId,
            "dry_corpse",
            "tsy",
            x,
            y,
            z,
            null,
            false,
            null,
            visualEntityId
        );
    }
}
