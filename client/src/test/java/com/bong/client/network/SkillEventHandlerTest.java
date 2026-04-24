package com.bong.client.network;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillRecentEventStore;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class SkillEventHandlerTest {
    @AfterEach
    void tearDown() {
        SkillSetStore.resetForTests();
        SkillRecentEventStore.resetForTests();
    }

    @Test
    void xpGainUpdatesEntryXpTotalAndRecentGain() {
        ServerDataDispatch dispatch = SkillEventHandler.xpGainHandler().handle(parseEnvelope(
            "{" +
                "\"v\":1," +
                "\"type\":\"skill_xp_gain\"," +
                "\"char_id\":1001," +
                "\"skill\":\"herbalism\"," +
                "\"amount\":5," +
                "\"source\":{\"type\":\"action\",\"plan_id\":\"botany\",\"action\":\"harvest_auto\"}" +
            "}"
        ));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        SkillSetSnapshot.Entry entry = SkillSetStore.snapshot().get(SkillId.HERBALISM);
        assertEquals(5L, entry.xp());
        assertEquals(5L, entry.totalXp());
        assertEquals(5L, entry.recentGainXp());
        assertEquals(1, SkillRecentEventStore.snapshot().size());
        assertEquals("+5 XP", SkillRecentEventStore.snapshot().get(0).text());
    }

    @Test
    void lvUpResetsCurrentXpAndUpdatesThreshold() {
        SkillSetStore.updateEntry(SkillId.ALCHEMY, new SkillSetSnapshot.Entry(2, 200, 900, 1400, 5, 0, 0));

        ServerDataDispatch dispatch = SkillEventHandler.lvUpHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"skill_lv_up\",\"char_id\":1001,\"skill\":\"alchemy\",\"new_lv\":3}"
        ));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        SkillSetSnapshot.Entry entry = SkillSetStore.snapshot().get(SkillId.ALCHEMY);
        assertEquals(3, entry.lv());
        assertEquals(0L, entry.xp());
        assertEquals(1600L, entry.xpToNext());
        assertEquals("升至 Lv.3", SkillRecentEventStore.snapshot().get(0).text());
    }

    @Test
    void capChangedUpdatesCapOnly() {
        SkillSetStore.updateEntry(SkillId.FORGING, new SkillSetSnapshot.Entry(6, 1200, 4900, 9000, 10, 0, 0));

        ServerDataDispatch dispatch = SkillEventHandler.capChangedHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"skill_cap_changed\",\"char_id\":1001,\"skill\":\"forging\",\"new_cap\":7}"
        ));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        SkillSetSnapshot.Entry entry = SkillSetStore.snapshot().get(SkillId.FORGING);
        assertEquals(6, entry.lv());
        assertEquals(7, entry.cap());
        assertEquals(1200L, entry.xp());
    }

    @Test
    void scrollUsedDuplicateMarksConsumedScrollWithoutChangingXp() {
        ServerDataDispatch dispatch = SkillEventHandler.scrollUsedHandler().handle(parseEnvelope(
            "{" +
                "\"v\":1," +
                "\"type\":\"skill_scroll_used\"," +
                "\"char_id\":1001," +
                "\"scroll_id\":\"scroll:bai_cao_tu_kao_can\"," +
                "\"skill\":\"herbalism\"," +
                "\"xp_granted\":0," +
                "\"was_duplicate\":true" +
            "}"
        ));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(SkillSetSnapshot.Entry.zero(), SkillSetStore.snapshot().get(SkillId.HERBALISM));
        assertTrue(SkillSetStore.snapshot().hasConsumedScroll("scroll:bai_cao_tu_kao_can"));
    }

    @Test
    void scrollUsedGainUpdatesRecentGainAndConsumedScroll() {
        ServerDataDispatch dispatch = SkillEventHandler.scrollUsedHandler().handle(parseEnvelope(
            "{" +
                "\"v\":1," +
                "\"type\":\"skill_scroll_used\"," +
                "\"char_id\":1001," +
                "\"scroll_id\":\"scroll:dan_huo_can\"," +
                "\"skill\":\"alchemy\"," +
                "\"xp_granted\":500," +
                "\"was_duplicate\":false" +
            "}"
        ));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        SkillSetSnapshot.Entry entry = SkillSetStore.snapshot().get(SkillId.ALCHEMY);
        assertEquals(500L, entry.recentGainXp());
        assertTrue(entry.recentGainMillis() > 0L);
        assertTrue(SkillSetStore.snapshot().hasConsumedScroll("scroll:dan_huo_can"));
        assertEquals("残卷顿悟 +500 XP", SkillRecentEventStore.snapshot().get(0).text());
    }

    @Test
    void xpGainInvalidPayloadBecomesSafeNoOp() {
        ServerDataDispatch dispatch = SkillEventHandler.xpGainHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"skill_xp_gain\",\"char_id\":1001,\"skill\":\"alchemy\"}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("skill/amount"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
