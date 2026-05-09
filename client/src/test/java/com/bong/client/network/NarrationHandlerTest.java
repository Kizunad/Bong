package com.bong.client.network;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NarrationHandlerTest {
    private final NarrationHandler handler = new NarrationHandler();

    @BeforeEach
    void resetEventStore() {
        UnifiedEventStore.resetForTests();
    }

    @Test
    void mapsKnownStylesToStyledChatMessages() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"narration","narrations":[
              {"scope":"broadcast","text":"天道示警","style":"system_warning"},
              {"scope":"broadcast","text":"灵息流转","style":"perception"},
              {"scope":"broadcast","text":"山门开启","style":"narration"},
              {"scope":"broadcast","text":"新纪元已至","style":"era_decree"}
            ]}
            """));

        assertTrue(dispatch.handled());
        assertEquals(4, dispatch.chatMessages().size());

        assertStyledMessage(dispatch.chatMessages().get(0), "[天道警示] 天道示警", Formatting.RED.getColorValue(), true);
        assertStyledMessage(dispatch.chatMessages().get(1), "[感知] 灵息流转", Formatting.GRAY.getColorValue(), false);
        assertStyledMessage(dispatch.chatMessages().get(2), "[叙事] 山门开启", Formatting.WHITE.getColorValue(), false);
        assertStyledMessage(dispatch.chatMessages().get(3), "[时代法旨] 新纪元已至", Formatting.GOLD.getColorValue(), true);

        assertEquals("新纪元已至", dispatch.narrationState().orElseThrow().text());
        assertEquals("era_decree", dispatch.toastNarrationState().orElseThrow().style().wireName());
    }

    @Test
    void emptyNarrationsReturnNoOp() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"narration\",\"narrations\":[]}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.chatMessages().isEmpty());
    }

    @Test
    void missingTextReturnsNoOpWhenNothingValidRemains() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"narration","narrations":[{"scope":"broadcast","style":"narration"}]}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.narrationState().isEmpty());
        assertTrue(dispatch.toastNarrationState().isEmpty());
    }

    @Test
    void unknownStyleDowngradesToPlainChatText() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"narration","narrations":[{"scope":"broadcast","text":"万象归一","style":"mystery_style"}]}
            """));

        Text message = dispatch.chatMessages().get(0);
        assertEquals("万象归一", message.getString());
        assertNotNull(dispatch.narrationState().orElseThrow());
        assertTrue(message.getSiblings().isEmpty());
        assertTrue(message.getStyle().getColor() == null);
        assertTrue(dispatch.toastNarrationState().isEmpty());
    }

    @Test
    void urgentPoliticalNarrationRoutesToChatHud() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"narration","narrations":[
              {"scope":"broadcast","text":"江湖有传，玄锋画影已过诸市。","style":"political_jianghu","kind":"political_jianghu"}
            ]}
            """));

        assertTrue(dispatch.handled());
        assertEquals(1, dispatch.chatMessages().size());
        assertStyledMessage(
            dispatch.chatMessages().get(0),
            "[江湖传闻] 江湖有传，玄锋画影已过诸市。",
            Formatting.DARK_AQUA.getColorValue(),
            false
        );
        assertEquals(0, UnifiedEventStore.stream().size());
    }

    @Test
    void nonUrgentPoliticalNarrationRoutesToEventStore() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"narration","narrations":[
              {"scope":"zone","target":"blood_valley","text":"江湖有传，血谷旧怨又添一笔。","style":"political_jianghu","kind":"political_jianghu"}
            ]}
            """));

        assertTrue(dispatch.handled());
        assertTrue(dispatch.chatMessages().isEmpty());
        assertEquals("江湖有传，血谷旧怨又添一笔。", dispatch.narrationState().orElseThrow().text());
        assertEquals(1, UnifiedEventStore.stream().size());

        UnifiedEvent event = UnifiedEventStore.stream().snapshot().get(0);
        assertEquals(UnifiedEvent.Channel.SOCIAL, event.channel());
        assertEquals(UnifiedEvent.Priority.P2_NORMAL, event.priority());
        assertEquals("political_jianghu:blood_valley", event.sourceTag());
        assertEquals("[江湖传闻] 江湖有传，血谷旧怨又添一笔。", event.text());
    }

    @Test
    void invalidEntriesBecomeSafeNoOp() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"narration\",\"narrations\":[42,true,{\"scope\":\"broadcast\",\"text\":\"   \"}]}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.chatMessages().isEmpty());
    }

    private static void assertStyledMessage(Text message, String expectedText, Integer expectedColor, boolean expectedBold) {
        assertEquals(expectedText, message.getString());
        assertNotNull(message.getStyle().getColor());
        assertEquals(expectedColor, message.getStyle().getColor().getRgb());
        assertEquals(expectedBold, message.getStyle().isBold());
        assertFalse(message.getSiblings().isEmpty());
        assertNotNull(message.getSiblings().get(0).getStyle().getColor());
        assertEquals(expectedColor, message.getSiblings().get(0).getStyle().getColor().getRgb());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), () -> "Expected payload to parse successfully but got: " + parseResult.errorMessage());
        return parseResult.envelope();
    }
}
