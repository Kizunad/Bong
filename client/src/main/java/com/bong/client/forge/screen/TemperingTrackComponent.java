package com.bong.client.forge.screen;

import com.bong.client.forge.state.ForgeSessionStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** plan-forge-leftovers-v1 §3.1 — 淬炼节奏轨道。 */
public class TemperingTrackComponent extends BaseComponent {
    public static final int TRACK_WIDTH = 260;
    public static final int TRACK_HEIGHT = 86;
    public static final int HIT_LINE_OFFSET = TRACK_WIDTH / 2;

    private static final int BG_COLOR = 0xCC10151B;
    private static final int BORDER_COLOR = 0xFF334150;
    private static final int TRACK_COLOR = 0xFF263442;
    private static final int HIT_LINE_COLOR = 0xFFFFF0A6;
    private static final int DONE_COLOR = 0xFF7FAA99;
    private static final int TEXT_COLOR = 0xFFD8E8F0;
    private static final int MUTED_TEXT_COLOR = 0xFF8A98A4;
    private static final int DEVIATION_SAFE = 0xFF44C777;
    private static final int DEVIATION_WARN = 0xFFFFCC55;
    private static final int DEVIATION_MAX = 0xFFFF5555;
    private static final int NOTE_SPACING = 28;
    private static final int NOTE_RADIUS = 5;
    private static final int MAX_DEVIATION_FOR_BAR = 8;

    public TemperingTrackComponent() {
        this.sizing(Sizing.fixed(TRACK_WIDTH), Sizing.fixed(TRACK_HEIGHT));
    }

    public static RenderState renderStateFrom(ForgeSessionStore.Snapshot snapshot) {
        if (snapshot == null || !"tempering".equals(snapshot.currentStep())) {
            return RenderState.empty();
        }
        JsonObject state = parseJsonObject(snapshot.stepStateJson());
        List<String> pattern = readPattern(state);
        int cursor = readNonNegativeInt(state, "beat_cursor", 0);
        int hits = readNonNegativeInt(state, "hits", 0);
        int misses = readNonNegativeInt(state, "misses", 0);
        int deviation = readNonNegativeInt(state, "deviation", 0);
        return new RenderState(remainingPattern(pattern, cursor), cursor, hits, misses, deviation);
    }

    public RenderState currentRenderState() {
        return renderStateFrom(ForgeSessionStore.snapshot());
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        drawTrack(context, currentRenderState(), x, y);
    }

    public static void drawTrack(OwoUIDrawContext context, RenderState state, int x, int y) {
        if (context == null) return;

        RenderState safeState = state == null ? RenderState.empty() : state;
        context.fill(x, y, x + TRACK_WIDTH, y + TRACK_HEIGHT, BG_COLOR);
        drawBorder(context, x, y, TRACK_WIDTH, TRACK_HEIGHT, BORDER_COLOR);

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        context.drawTextWithShadow(textRenderer, Text.literal("淬炼节奏  J=Light  K=Heavy  L=Fold"),
            x + 8, y + 7, TEXT_COLOR);

        int trackY = y + 38;
        context.fill(x + 12, trackY - 1, x + TRACK_WIDTH - 12, trackY + 2, TRACK_COLOR);
        int hitX = x + HIT_LINE_OFFSET;
        context.fill(hitX, trackY - 18, hitX + 1, trackY + 19, HIT_LINE_COLOR);

        if (safeState.patternRemaining().isEmpty()) {
            String done = "淬炼节拍已完成";
            int doneX = x + (TRACK_WIDTH - textRenderer.getWidth(done)) / 2;
            context.drawTextWithShadow(textRenderer, Text.literal(done), doneX, trackY - 5, DONE_COLOR);
        } else {
            for (int i = 0; i < safeState.patternRemaining().size(); i++) {
                String beat = safeState.patternRemaining().get(i);
                int noteX = x + TRACK_WIDTH - 28 - i * NOTE_SPACING;
                if (noteX < x + 16 || noteX > x + TRACK_WIDTH - 16) continue;
                drawNote(context, textRenderer, beat, noteX, trackY);
            }
        }

        String combo = "combo " + safeState.combo() + "  miss " + safeState.misses();
        context.drawTextWithShadow(textRenderer, Text.literal(combo), x + 8, y + 57, MUTED_TEXT_COLOR);
        drawDeviationBar(context, safeState.deviation(), x + 126, y + 60, 116, 6);
    }

    private static void drawNote(OwoUIDrawContext context, TextRenderer textRenderer, String beat, int cx, int cy) {
        int color = beatColor(beat);
        context.fill(cx - NOTE_RADIUS, cy - NOTE_RADIUS, cx + NOTE_RADIUS + 1, cy + NOTE_RADIUS + 1, color);
        drawBorder(context, cx - NOTE_RADIUS, cy - NOTE_RADIUS, NOTE_RADIUS * 2 + 1, NOTE_RADIUS * 2 + 1, 0xEE000000);
        int labelColor = "F".equals(beat) ? 0xFF3A2B00 : 0xFFFFFFFF;
        context.drawTextWithShadow(textRenderer, Text.literal(beat), cx - 3, cy - 4, labelColor);
    }

    private static void drawDeviationBar(OwoUIDrawContext context, int deviation, int x, int y, int w, int h) {
        context.fill(x, y, x + w, y + h, 0xFF1E252B);
        int clamped = Math.max(0, Math.min(MAX_DEVIATION_FOR_BAR, deviation));
        int fill = (int) Math.round(w * (clamped / (double) MAX_DEVIATION_FOR_BAR));
        if (fill > 0) {
            context.fill(x, y, x + fill, y + h, deviationColor(deviation));
        }
        drawBorder(context, x, y, w, h, 0xFF3A4652);
    }

    private static void drawBorder(OwoUIDrawContext context, int x, int y, int w, int h, int color) {
        context.fill(x, y, x + w, y + 1, color);
        context.fill(x, y + h - 1, x + w, y + h, color);
        context.fill(x, y + 1, x + 1, y + h - 1, color);
        context.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    public static int beatColor(String beat) {
        return switch (normalizeBeat(beat)) {
            case "H" -> 0xFFE64B4B;
            case "F" -> 0xFFF2CA4C;
            default -> 0xFF49A7FF;
        };
    }

    public static int deviationColor(int deviation) {
        if (deviation >= MAX_DEVIATION_FOR_BAR) return DEVIATION_MAX;
        if (deviation >= MAX_DEVIATION_FOR_BAR / 2) return DEVIATION_WARN;
        return DEVIATION_SAFE;
    }

    private static JsonObject parseJsonObject(String json) {
        if (json == null || json.isBlank()) return new JsonObject();
        try {
            JsonElement parsed = JsonParser.parseString(json);
            return parsed != null && parsed.isJsonObject() ? parsed.getAsJsonObject() : new JsonObject();
        } catch (RuntimeException ignored) {
            return new JsonObject();
        }
    }

    private static List<String> readPattern(JsonObject state) {
        JsonArray raw = readArray(state, "pattern_remaining");
        if (raw == null) raw = readArray(state, "pattern");
        if (raw == null) return List.of();

        List<String> beats = new ArrayList<>();
        for (JsonElement element : raw) {
            if (element == null || !element.isJsonPrimitive()) continue;
            String beat = normalizeBeat(element.getAsString());
            if (!beat.isEmpty()) beats.add(beat);
        }
        return beats;
    }

    private static JsonArray readArray(JsonObject state, String key) {
        if (state == null || !state.has(key) || !state.get(key).isJsonArray()) return null;
        return state.getAsJsonArray(key);
    }

    private static int readNonNegativeInt(JsonObject state, String key, int fallback) {
        if (state == null || !state.has(key)) return Math.max(0, fallback);
        try {
            return Math.max(0, state.get(key).getAsInt());
        } catch (RuntimeException ignored) {
            return Math.max(0, fallback);
        }
    }

    private static List<String> remainingPattern(List<String> pattern, int cursor) {
        if (pattern.isEmpty()) return List.of();
        if (cursor <= 0) return List.copyOf(pattern);
        if (cursor >= pattern.size()) return List.of();
        return List.copyOf(pattern.subList(cursor, pattern.size()));
    }

    private static String normalizeBeat(String beat) {
        if (beat == null) return "";
        return switch (beat.trim().toUpperCase(Locale.ROOT)) {
            case "L", "LIGHT" -> "L";
            case "H", "HEAVY" -> "H";
            case "F", "FOLD" -> "F";
            default -> "";
        };
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) {
        return TRACK_WIDTH;
    }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) {
        return TRACK_HEIGHT;
    }

    public record RenderState(List<String> patternRemaining, int beatCursor, int hits, int misses, int deviation) {
        public RenderState {
            patternRemaining = patternRemaining == null ? List.of() : List.copyOf(patternRemaining);
            beatCursor = Math.max(0, beatCursor);
            hits = Math.max(0, hits);
            misses = Math.max(0, misses);
            deviation = Math.max(0, deviation);
        }

        public static RenderState empty() {
            return new RenderState(List.of(), 0, 0, 0, 0);
        }

        public int combo() {
            return hits;
        }
    }
}
