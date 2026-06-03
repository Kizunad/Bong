package com.bong.client.combat.baomai.v4;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — CrackReadingHandler + HudStateStore 单测。
 *
 * 测契约：
 *   - 解析完整 JSON → CrackReadingHudStateStore 字段匹配
 *   - 深读 is_deep=true → isDeep=true 写入
 *   - display_ticks → expiresAtMs 超期后 snapshot=EMPTY
 *   - entries[] 解析（meridian / bracket / has_circuit / is_dead_armor）
 *   - 无效 JSON → 不崩溃，store 维持 EMPTY
 *   - 空 entries → 写入 empty list
 */
public class CrackReadingHandlerTest {

    @BeforeEach
    void setUp() {
        CrackReadingHudStateStore.clear();
    }

    @AfterEach
    void tearDown() {
        CrackReadingHudStateStore.clear();
    }

    @Test
    void handle_validPayload_storeUpdated() {
        String json = """
                {
                  "target_entity_id": 123456789,
                  "entries": [
                    {"meridian": "Lung", "bracket": "MicroTear", "has_circuit": true, "is_dead_armor": false},
                    {"meridian": "Heart", "bracket": "Severed", "has_circuit": false, "is_dead_armor": true}
                  ],
                  "is_deep": true,
                  "display_ticks": 60
                }
                """;

        CrackReadingHandler.handle(json);

        CrackReadingHudStateStore.State state = CrackReadingHudStateStore.snapshot(System.currentTimeMillis());
        assertTrue(state.isActive(), "store should be active after valid payload");
        assertEquals(123456789L, state.targetEntityId,
                "target_entity_id should match — client uses this to identify target");
        assertTrue(state.isDeep, "is_deep=true should be parsed correctly");
        assertEquals(2, state.entries.size(),
                "entries[] should have exactly 2 elements");

        CrackReadingHudStateStore.MeridianEntry lung = state.entries.get(0);
        assertEquals("Lung", lung.meridian(), "first entry meridian should be 'Lung'");
        assertEquals("MicroTear", lung.bracket(), "first entry bracket should be 'MicroTear'");
        assertTrue(lung.hasCircuit(), "has_circuit=true should be parsed");
        assertFalse(lung.isDeadArmor(), "is_dead_armor=false should be parsed");

        CrackReadingHudStateStore.MeridianEntry heart = state.entries.get(1);
        assertEquals("Heart", heart.meridian());
        assertEquals("Severed", heart.bracket());
        assertFalse(heart.hasCircuit());
        assertTrue(heart.isDeadArmor(), "is_dead_armor=true should be parsed");
    }

    @Test
    void handle_shallowRead_isDeepFalse() {
        String json = """
                {"target_entity_id": 1, "entries": [], "is_deep": false, "display_ticks": 30}
                """;
        CrackReadingHandler.handle(json);
        CrackReadingHudStateStore.State state = CrackReadingHudStateStore.snapshot();
        assertFalse(state.isDeep, "shallow read (is_deep=false) should be stored correctly");
    }

    @Test
    void handle_displayTicksExpiry() {
        // display_ticks=1 → 50ms 后过期
        String json = """
                {"target_entity_id": 42, "entries": [], "is_deep": false, "display_ticks": 1}
                """;
        CrackReadingHandler.handle(json);

        // 在 50ms 内仍活跃
        CrackReadingHudStateStore.State activeState = CrackReadingHudStateStore.snapshot(0L);
        assertTrue(activeState.isActive(), "state should be active immediately after accept");

        // 超期后 snapshot 返回 EMPTY
        CrackReadingHudStateStore.State expiredState = CrackReadingHudStateStore.snapshot(Long.MAX_VALUE);
        assertFalse(expiredState.isActive(), "state should expire after display_ticks ms passed");
        assertSame(CrackReadingHudStateStore.State.EMPTY, expiredState,
                "expired state should be State.EMPTY sentinel");
    }

    @Test
    void handle_invalidJson_doesNotCrash_storeRemainsEmpty() {
        CrackReadingHandler.handle("not-json");
        CrackReadingHudStateStore.State state = CrackReadingHudStateStore.snapshot();
        assertFalse(state.isActive(), "invalid JSON should not update store, state must remain EMPTY");
    }

    @Test
    void handle_emptyJson_doesNotCrash() {
        CrackReadingHandler.handle("{}");
        // no crash = pass
    }

    @Test
    void handle_emptyEntries_storedAsEmptyList() {
        String json = """
                {"target_entity_id": 99, "entries": [], "is_deep": false, "display_ticks": 60}
                """;
        CrackReadingHandler.handle(json);
        CrackReadingHudStateStore.State state = CrackReadingHudStateStore.snapshot();
        assertTrue(state.isActive());
        assertEquals(0, state.entries.size(), "empty entries array should store as empty list");
    }

    @Test
    void handle_entriesImmutable() {
        String json = """
                {"target_entity_id": 5, "entries": [{"meridian":"Du","bracket":"Torn","has_circuit":false,"is_dead_armor":false}], "is_deep": false, "display_ticks": 100}
                """;
        CrackReadingHandler.handle(json);
        CrackReadingHudStateStore.State state = CrackReadingHudStateStore.snapshot();
        assertThrows(UnsupportedOperationException.class,
                () -> state.entries.add(new CrackReadingHudStateStore.MeridianEntry("X", "Intact", false, false)),
                "entries list should be immutable — mutation must throw");
    }

    @Test
    void clear_resetsToEmpty() {
        String json = """
                {"target_entity_id": 1, "entries": [], "is_deep": false, "display_ticks": 1000}
                """;
        CrackReadingHandler.handle(json);
        assertTrue(CrackReadingHudStateStore.snapshot().isActive(), "should be active before clear");
        CrackReadingHudStateStore.clear();
        assertFalse(CrackReadingHudStateStore.snapshot().isActive(), "should be EMPTY after clear");
    }
}
