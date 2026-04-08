package com.bong.client.network.handlers;

import net.minecraft.text.Text;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NarrationHandlerTest {
    @Test
    void handlePayloadRoutesSupportedStylesToChatAndPriorityToasts() {
        NarrationHandler handler = new NarrationHandler();
        RecordingNarrationOutput output = new RecordingNarrationOutput();
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"narration\"," +
            "\"narrations\":[" +
            "{\"scope\":\"broadcast\",\"text\":\"晨雾压城\",\"style\":\"narration\"}," +
            "{\"scope\":\"broadcast\",\"text\":\"你听见远处雷声\",\"style\":\"perception\"}," +
            "{\"scope\":\"broadcast\",\"text\":\"天劫将至\",\"style\":\"system_warning\"}," +
            "{\"scope\":\"broadcast\",\"text\":\"新纪元已启\",\"style\":\"era_decree\"}" +
            "]}";

        handler.handlePayload(json, output);

        assertEquals(
            List.of("[叙事] 晨雾压城", "[感知] 你听见远处雷声", "[天道警示] 天劫将至", "[时代] 新纪元已启"),
            output.chatMessages.stream().map(Text::getString).toList()
        );
        assertEquals(2, output.toasts.size());
        assertEquals("天劫将至", output.toasts.get(0).text());
        assertEquals(0xFF5555, output.toasts.get(0).color());
        assertEquals(5_000L, output.toasts.get(0).durationMs());
        assertEquals("新纪元已启", output.toasts.get(1).text());
        assertEquals(0xFFAA00, output.toasts.get(1).color());
        assertEquals(8_000L, output.toasts.get(1).durationMs());
    }

    @Test
    void buildChatMessageAppliesWarningFormatting() {
        NarrationPayloadParser.RenderedNarration warning = new NarrationPayloadParser.RenderedNarration(
            "system_warning",
            "天劫将至",
            "[天道警示]",
            0xFF5555,
            0xFF5555,
            true,
            new NarrationPayloadParser.ToastSpec("天劫将至", 0xFF5555, 5_000L)
        );

        Text message = NarrationHandler.buildChatMessage(warning);

        assertEquals("[天道警示] 天劫将至", message.getString());
        assertEquals(3, message.getSiblings().size());
        assertEquals(0xFF5555, message.getSiblings().get(0).getStyle().getColor().getRgb());
        assertTrue(message.getSiblings().get(0).getStyle().isBold());
        assertEquals(0xFF5555, message.getSiblings().get(2).getStyle().getColor().getRgb());
    }

    @Test
    void handlePayloadFallsBackForUnknownStyleWithoutToast() {
        NarrationHandler handler = new NarrationHandler();
        RecordingNarrationOutput output = new RecordingNarrationOutput();
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"narration\"," +
            "\"narrations\":[" +
            "{\"scope\":\"broadcast\",\"text\":\"未定义风格\",\"style\":\"heavenly_static\"}" +
            "]}";

        handler.handlePayload(json, output);

        assertEquals(List.of("未定义风格"), output.chatMessages.stream().map(Text::getString).toList());
        assertTrue(output.toasts.isEmpty());
    }

    @Test
    void handlePayloadIgnoresMalformedPayloadSafely() {
        NarrationHandler handler = new NarrationHandler();
        RecordingNarrationOutput output = new RecordingNarrationOutput();
        String json = "{\"v\":1,\"type\":\"narration\",\"narrations\":[{\"text\":\"破损\",\"style\":\"narration\"}";

        handler.handlePayload(json, output);

        assertTrue(output.chatMessages.isEmpty());
        assertTrue(output.toasts.isEmpty());
    }

    @Test
    void handlePayloadSupportsGameplayHintsUsingExistingNarrationPath() {
        NarrationHandler handler = new NarrationHandler();
        RecordingNarrationOutput output = new RecordingNarrationOutput();
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"narration\"," +
            "\"narrations\":[" +
            "{\"scope\":\"player\",\"target\":\"offline:Azure\",\"text\":\"你采得 spirit_herb，储物与阅历皆有所增长。\",\"style\":\"narration\"}," +
            "{\"scope\":\"player\",\"target\":\"offline:Azure\",\"text\":\"你已突破至 炼气一层，灵海扩张至 120/120。\",\"style\":\"system_warning\"}" +
            "]}";

        handler.handlePayload(json, output);

        assertEquals(
            List.of("[叙事] 你采得 spirit_herb，储物与阅历皆有所增长。", "[天道警示] 你已突破至 炼气一层，灵海扩张至 120/120。"),
            output.chatMessages.stream().map(Text::getString).toList()
        );
        assertEquals(1, output.toasts.size());
        assertEquals("你已突破至 炼气一层，灵海扩张至 120/120。", output.toasts.get(0).text());
        assertEquals(0xFF5555, output.toasts.get(0).color());
    }

    private static final class RecordingNarrationOutput implements NarrationHandler.NarrationOutput {
        private final List<Text> chatMessages = new ArrayList<>();
        private final List<NarrationPayloadParser.ToastSpec> toasts = new ArrayList<>();

        @Override
        public void sendChat(Text message) {
            chatMessages.add(message);
        }

        @Override
        public void showToast(NarrationPayloadParser.ToastSpec toast) {
            toasts.add(toast);
        }
    }
}
