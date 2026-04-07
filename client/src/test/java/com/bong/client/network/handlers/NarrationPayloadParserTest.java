package com.bong.client.network.handlers;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NarrationPayloadParserTest {
    @Test
    void parsesSupportedStylesIntoChatAndToastRules() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"narration\"," +
            "\"narrations\":[" +
            "{\"scope\":\"broadcast\",\"text\":\"晨雾压城\",\"style\":\"narration\"}," +
            "{\"scope\":\"broadcast\",\"text\":\"你听见远处雷声\",\"style\":\"perception\"}," +
            "{\"scope\":\"broadcast\",\"text\":\"天劫将至\",\"style\":\"system_warning\"}," +
            "{\"scope\":\"broadcast\",\"text\":\"新纪元已启\",\"style\":\"era_decree\"}" +
            "]}";

        NarrationPayloadParser.ParseResult result = NarrationPayloadParser.parse(json);

        assertTrue(result.success());
        assertEquals(4, result.narrations().size());

        NarrationPayloadParser.RenderedNarration narration = result.narrations().get(0);
        assertEquals("[叙事] 晨雾压城", narration.plainChatText());
        assertNull(narration.toast());

        NarrationPayloadParser.RenderedNarration perception = result.narrations().get(1);
        assertEquals("[感知] 你听见远处雷声", perception.plainChatText());
        assertNull(perception.toast());

        NarrationPayloadParser.RenderedNarration warning = result.narrations().get(2);
        assertEquals("[天道警示] 天劫将至", warning.plainChatText());
        assertNotNull(warning.toast());
        assertEquals("天劫将至", warning.toast().text());
        assertEquals(0xFF5555, warning.toast().color());
        assertEquals(5_000L, warning.toast().durationMs());

        NarrationPayloadParser.RenderedNarration eraDecree = result.narrations().get(3);
        assertEquals("[时代] 新纪元已启", eraDecree.plainChatText());
        assertNotNull(eraDecree.toast());
        assertEquals("新纪元已启", eraDecree.toast().text());
        assertEquals(0xFFAA00, eraDecree.toast().color());
        assertEquals(8_000L, eraDecree.toast().durationMs());
    }

    @Test
    void unknownStyleFallsBackToPlainChatWithoutToast() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"narration\"," +
            "\"narrations\":[" +
            "{\"scope\":\"broadcast\",\"text\":\"未定义风格\",\"style\":\"heavenly_static\"}" +
            "]}";

        NarrationPayloadParser.ParseResult result = NarrationPayloadParser.parse(json);

        assertTrue(result.success());
        assertEquals(1, result.narrations().size());
        assertEquals("heavenly_static", result.narrations().get(0).style());
        assertEquals("未定义风格", result.narrations().get(0).plainChatText());
        assertNull(result.narrations().get(0).toast());
    }

    @Test
    void malformedPayloadFailsSafely() {
        String json = "{\"v\":1,\"type\":\"narration\",\"narrations\":[{\"text\":\"破损\",\"style\":\"narration\"}";

        NarrationPayloadParser.ParseResult result = NarrationPayloadParser.parse(json);

        assertFalse(result.success());
        assertTrue(result.narrations().isEmpty());
        assertNotNull(result.errorMessage());
        assertEquals("Malformed narration payload", result.errorMessage());
    }

    @Test
    void missingNarrationsArrayFailsSafely() {
        String json = "{\"v\":1,\"type\":\"narration\"}";

        NarrationPayloadParser.ParseResult result = NarrationPayloadParser.parse(json);

        assertFalse(result.success());
        assertTrue(result.narrations().isEmpty());
        assertEquals("Narration payload missing array field 'narrations'", result.errorMessage());
    }

    @Test
    void invalidNarrationEntriesAreSkippedSafely() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"narration\"," +
            "\"narrations\":[" +
            "123," +
            "{\"scope\":\"broadcast\",\"style\":\"narration\"}," +
            "{\"scope\":\"broadcast\",\"text\":\"仍然有效\",\"style\":\"narration\"}" +
            "]}";

        NarrationPayloadParser.ParseResult result = NarrationPayloadParser.parse(json);

        assertTrue(result.success());
        assertEquals(1, result.narrations().size());
        assertEquals("[叙事] 仍然有效", result.narrations().get(0).plainChatText());
    }
}
