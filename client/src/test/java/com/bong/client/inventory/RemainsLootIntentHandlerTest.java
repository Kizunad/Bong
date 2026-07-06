package com.bong.client.inventory;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.inventory.state.RemainsStore;
import com.bong.client.network.ClientRequestSender;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-remains-suite P0 — G 键遗骸拾取候选/派发单测。
 */
class RemainsLootIntentHandlerTest {

    @AfterEach
    void tearDown() {
        RemainsStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    @Test
    void candidateReturnsEmptyWhenClientIsNull() {
        Optional<InteractCandidate> result = new RemainsLootIntentHandler().candidate(null);

        assertFalse(result.isPresent(), "candidate(null) 应返回 empty，避免 headless/null client NPE");
    }

    @Test
    void candidateAtReturnsNearestEntryWithStableLabel() {
        RemainsStore.putOrReplace(entry("far", 10.0, 64.0, 10.0));
        RemainsStore.putOrReplace(entry("near", 1.0, 64.0, 0.0));

        Optional<InteractCandidate> result = RemainsLootIntentHandler.candidateAt(0.0, 64.0, 0.0);

        assertTrue(result.isPresent(), "附近有遗骸时应产出 LootRemains candidate");
        InteractCandidate candidate = result.get();
        assertEquals(InteractIntent.LootRemains, candidate.intent());
        assertEquals(ReservedInteractionIntents.LOOT_REMAINS_PRIORITY, candidate.priority());
        assertEquals(1.0, candidate.distanceSq());
        assertEquals("remains:near", candidate.debugLabel());
    }

    @Test
    void candidateAtReturnsEmptyWhenStoreIsEmpty() {
        Optional<InteractCandidate> result = RemainsLootIntentHandler.candidateAt(0.0, 64.0, 0.0);

        assertFalse(result.isPresent(), "store 为空时遗骸拾取不应参与 G 键优先级竞争");
    }

    @Test
    void candidateAtUsesStoreTieBreakerAcrossThreeEntries() {
        RemainsStore.putOrReplace(entry("first", 3.0, 64.0, 4.0));
        RemainsStore.putOrReplace(entry("second", 4.0, 64.0, 3.0));
        RemainsStore.putOrReplace(entry("third", 0.0, 64.0, 5.0));

        Optional<InteractCandidate> result = RemainsLootIntentHandler.candidateAt(0.0, 64.0, 0.0);

        assertTrue(result.isPresent(), "三具等距遗骸时仍应稳定产出 candidate");
        assertEquals(
            "remains:third",
            result.get().debugLabel(),
            "三具等距遗骸应沿用 RemainsStore 的 latest-insertion tie-breaker"
        );
    }

    @Test
    void remainsIdFromCandidateParsesOnlyLootRemainsLabels() {
        InteractCandidate valid = InteractCandidate.of(
            InteractIntent.LootRemains,
            1,
            1.0,
            "remains:uuid-a"
        );
        InteractCandidate wrongIntent = InteractCandidate.of(
            InteractIntent.PickupDroppedItem,
            1,
            1.0,
            "remains:uuid-a"
        );
        InteractCandidate wrongPrefix = InteractCandidate.of(
            InteractIntent.LootRemains,
            1,
            1.0,
            "dropped_loot:42"
        );
        InteractCandidate emptyId = InteractCandidate.of(
            InteractIntent.LootRemains,
            1,
            1.0,
            "remains:"
        );

        assertEquals("uuid-a", RemainsLootIntentHandler.remainsIdFromCandidate(valid));
        assertNull(RemainsLootIntentHandler.remainsIdFromCandidate(wrongIntent));
        assertNull(RemainsLootIntentHandler.remainsIdFromCandidate(wrongPrefix));
        assertNull(RemainsLootIntentHandler.remainsIdFromCandidate(emptyId));
        assertNull(RemainsLootIntentHandler.remainsIdFromCandidate(null));
    }

    @Test
    void dispatchSendsCandidateIdEvenWhenStoreChangesBeforeDispatch() {
        RemainsLootIntentHandler handler = new RemainsLootIntentHandler();
        RemainsStore.putOrReplace(entry("selected", 1.0, 64.0, 0.0));
        InteractCandidate candidate = RemainsLootIntentHandler
            .candidateAt(0.0, 64.0, 0.0)
            .orElseThrow();
        RemainsStore.replaceAll(List.of(entry("new-nearest", 0.1, 64.0, 0.0)));
        List<String> sent = new ArrayList<>();
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sent.add(new String(payload, StandardCharsets.UTF_8))
        );

        boolean dispatched = handler.dispatch(null, candidate);

        assertTrue(dispatched, "有效候选应派发 remains_loot 请求");
        assertEquals(1, sent.size(), "一次 dispatch 应只发一个 client_request payload");
        assertTrue(
            sent.get(0).contains("\"remains_id\":\"selected\""),
            "dispatch 必须使用 candidate 已选中的 remains_id，不能在 store 更新后重选；actual=" + sent.get(0)
        );
    }

    @Test
    void dispatchReturnsFalseForMalformedCandidate() {
        InteractCandidate malformed = InteractCandidate.of(
            InteractIntent.LootRemains,
            1,
            1.0,
            "remains:"
        );

        boolean dispatched = new RemainsLootIntentHandler().dispatch(null, malformed);

        assertFalse(dispatched, "缺少 remains_id 的候选不应发包");
    }

    private static RemainsStore.Entry entry(String id, double x, double y, double z) {
        return new RemainsStore.Entry(id, x, y, z, "minecraft:overworld", "遗骸", 1, 0L);
    }
}
