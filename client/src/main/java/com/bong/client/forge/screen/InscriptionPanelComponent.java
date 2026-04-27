package com.bong.client.forge.screen;

import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Size;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** plan-forge-leftovers-v1 §4.1 — 铭文槽与残卷投放面板。 */
public class InscriptionPanelComponent extends BaseComponent {
    public static final int PANEL_WIDTH = 260;
    public static final int PANEL_HEIGHT = 104;
    public static final int SLOT_SIZE = 24;

    private static final int BG_COLOR = 0xCC15101B;
    private static final int BORDER_COLOR = 0xFF493856;
    private static final int SLOT_BG = 0xFF201829;
    private static final int SLOT_BORDER = 0xFF765A8A;
    private static final int SLOT_FILLED = 0xFF5C3F8A;
    private static final int VALID_OUTLINE = 0xFF88D080;
    private static final int TEXT_COLOR = 0xFFE8D8F4;
    private static final int MUTED_TEXT_COLOR = 0xFF9A88AA;
    private static final int FAIL_COLOR = 0xFFFF7777;
    private static final String INSCRIPTION_SCROLL_KIND = "inscription_scroll";

    private final List<String> acceptedInscriptionIds = new ArrayList<>();

    public InscriptionPanelComponent() {
        this.sizing(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(PANEL_HEIGHT));
        this.inflate(Size.of(PANEL_WIDTH, PANEL_HEIGHT));
    }

    public void placeAt(int screenX, int screenY) {
        this.moveTo(screenX, screenY);
        if (this.width == 0 || this.height == 0) {
            this.inflate(Size.of(PANEL_WIDTH, PANEL_HEIGHT));
        }
    }

    public RenderState currentRenderState() {
        RenderState base = renderStateFrom(ForgeSessionStore.snapshot());
        if (acceptedInscriptionIds.isEmpty()) return base;

        List<String> merged = new ArrayList<>(base.filledSlots());
        for (String id : acceptedInscriptionIds) {
            if (id != null && !id.isBlank()) merged.add(id);
        }
        int maxSlots = Math.max(base.maxSlots(), merged.size());
        return new RenderState(maxSlots, merged, base.failed(), base.failChanceRemaining());
    }

    public static RenderState renderStateFrom(ForgeSessionStore.Snapshot snapshot) {
        if (snapshot == null || !"inscription".equals(snapshot.currentStep())) {
            return RenderState.empty();
        }

        JsonObject state = parseJsonObject(snapshot.stepStateJson());
        SlotReadResult slots = readSlots(state);
        int filled = readNonNegativeInt(state, "filled_slots", slots.filledIds().size());
        int maxSlots = readNonNegativeInt(state, "max_slots", slots.slotCount());
        if (slots.slotCount() > 0) maxSlots = slots.slotCount();

        List<String> filledIds = new ArrayList<>(slots.filledIds());
        while (filledIds.size() < filled) {
            filledIds.add("slot_" + (filledIds.size() + 1));
        }

        maxSlots = Math.max(1, Math.max(maxSlots, filledIds.size()));
        boolean failed = readBoolean(state, "failed", false);
        Double failChance = readOptionalUnitDouble(state, "fail_chance_remaining");
        if (failChance == null) failChance = readOptionalUnitDouble(state, "fail_chance");
        return new RenderState(maxSlots, filledIds, failed, failChance);
    }

    public boolean tryDropScroll(InventoryItem scroll) {
        return onScrollDropped(scroll);
    }

    private boolean onScrollDropped(InventoryItem scroll) {
        ForgeSessionStore.Snapshot session = ForgeSessionStore.snapshot();
        if (session.sessionId() <= 0 || !"inscription".equals(session.currentStep())) {
            return false;
        }
        if (scroll == null || !scroll.isInscriptionScroll()) {
            return false;
        }

        String inscriptionId = scroll.inscriptionId();
        if (inscriptionId.isBlank()) {
            return false;
        }

        RenderState state = currentRenderState();
        if (state.filledCount() >= state.maxSlots()) {
            return false;
        }

        ClientRequestSender.sendForgeInscriptionScroll(session.sessionId(), inscriptionId);
        acceptedInscriptionIds.add(inscriptionId);
        return true;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        drawPanel(context, currentRenderState(), x, y, false);
    }

    public static void drawPanel(OwoUIDrawContext context, RenderState state, int x, int y, boolean dropHover) {
        if (context == null) return;

        RenderState safeState = state == null ? RenderState.empty() : state;
        context.fill(x, y, x + PANEL_WIDTH, y + PANEL_HEIGHT, BG_COLOR);
        drawBorder(context, x, y, PANEL_WIDTH, PANEL_HEIGHT, dropHover ? VALID_OUTLINE : BORDER_COLOR);

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        context.drawTextWithShadow(textRenderer, Text.literal("铭文槽"), x + 8, y + 7, TEXT_COLOR);
        context.drawTextWithShadow(textRenderer, Text.literal(safeState.failChanceLabel()),
            x + 154, y + 7, safeState.failed() ? FAIL_COLOR : MUTED_TEXT_COLOR);

        int slotY = y + 32;
        int totalWidth = safeState.maxSlots() * SLOT_SIZE + Math.max(0, safeState.maxSlots() - 1) * 10;
        int slotX = x + Math.max(8, (PANEL_WIDTH - totalWidth) / 2);
        for (int i = 0; i < safeState.maxSlots(); i++) {
            String filled = safeState.inscriptionAt(i);
            int sx = slotX + i * (SLOT_SIZE + 10);
            drawSlot(context, textRenderer, sx, slotY, filled);
        }

        String hint = dropHover ? "松手投入铭文残卷" : "从右侧背包拖入 inscription_scroll_*";
        int hintX = x + (PANEL_WIDTH - textRenderer.getWidth(hint)) / 2;
        context.drawTextWithShadow(textRenderer, Text.literal(hint), hintX, y + 76,
            dropHover ? VALID_OUTLINE : MUTED_TEXT_COLOR);
    }

    private static void drawSlot(OwoUIDrawContext context, TextRenderer textRenderer, int x, int y, String inscriptionId) {
        boolean filled = inscriptionId != null && !inscriptionId.isBlank();
        context.fill(x, y, x + SLOT_SIZE, y + SLOT_SIZE, filled ? SLOT_FILLED : SLOT_BG);
        drawBorder(context, x, y, SLOT_SIZE, SLOT_SIZE, SLOT_BORDER);
        if (filled) {
            String label = compactInscriptionLabel(inscriptionId);
            int lx = x + (SLOT_SIZE - textRenderer.getWidth(label)) / 2;
            context.drawTextWithShadow(textRenderer, Text.literal(label), lx, y + 8, 0xFFFFFFFF);
        } else {
            context.drawTextWithShadow(textRenderer, Text.literal("+"), x + 9, y + 7, MUTED_TEXT_COLOR);
        }
    }

    private static String compactInscriptionLabel(String id) {
        if (id == null || id.isBlank()) return "?";
        String[] parts = id.split("_");
        String last = parts.length == 0 ? id : parts[0];
        return last.isBlank() ? "?" : last.substring(0, Math.min(3, last.length())).toUpperCase(Locale.ROOT);
    }

    private static void drawBorder(OwoUIDrawContext context, int x, int y, int w, int h, int color) {
        context.fill(x, y, x + w, y + 1, color);
        context.fill(x, y + h - 1, x + w, y + h, color);
        context.fill(x, y + 1, x + 1, y + h - 1, color);
        context.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
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

    private static SlotReadResult readSlots(JsonObject state) {
        JsonArray raw = readArray(state, "slots");
        if (raw == null) raw = readArray(state, "filled_inscriptions");
        if (raw == null) raw = readArray(state, "inscriptions");
        if (raw == null) return new SlotReadResult(0, List.of());

        List<String> filled = new ArrayList<>();
        for (JsonElement element : raw) {
            String id = readInscriptionId(element);
            if (!id.isBlank()) filled.add(id);
        }
        return new SlotReadResult(raw.size(), filled);
    }

    private static JsonArray readArray(JsonObject state, String key) {
        if (state == null || !state.has(key) || !state.get(key).isJsonArray()) return null;
        return state.getAsJsonArray(key);
    }

    private static String readInscriptionId(JsonElement element) {
        if (element == null || element.isJsonNull()) return "";
        try {
            if (element.isJsonPrimitive()) return element.getAsString().trim();
            if (!element.isJsonObject()) return "";
            JsonObject object = element.getAsJsonObject();
            for (String key : List.of("inscription_id", "id", "item_id")) {
                if (object.has(key) && object.get(key).isJsonPrimitive()) {
                    return object.get(key).getAsString().trim();
                }
            }
        } catch (RuntimeException ignored) {
            return "";
        }
        return "";
    }

    private static int readNonNegativeInt(JsonObject state, String key, int fallback) {
        if (state == null || !state.has(key)) return Math.max(0, fallback);
        try {
            return Math.max(0, state.get(key).getAsInt());
        } catch (RuntimeException ignored) {
            return Math.max(0, fallback);
        }
    }

    private static boolean readBoolean(JsonObject state, String key, boolean fallback) {
        if (state == null || !state.has(key)) return fallback;
        try {
            return state.get(key).getAsBoolean();
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static Double readOptionalUnitDouble(JsonObject state, String key) {
        if (state == null || !state.has(key)) return null;
        try {
            double value = state.get(key).getAsDouble();
            if (!Double.isFinite(value)) return null;
            return Math.max(0.0, Math.min(1.0, value));
        } catch (RuntimeException ignored) {
            return null;
        }
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) {
        return PANEL_WIDTH;
    }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) {
        return PANEL_HEIGHT;
    }

    private record SlotReadResult(int slotCount, List<String> filledIds) {
        private SlotReadResult {
            filledIds = filledIds == null ? List.of() : List.copyOf(filledIds);
            slotCount = Math.max(0, slotCount);
        }
    }

    public record RenderState(int maxSlots, List<String> filledSlots, boolean failed, Double failChanceRemaining) {
        public RenderState {
            maxSlots = Math.max(0, maxSlots);
            filledSlots = filledSlots == null ? List.of() : List.copyOf(filledSlots);
            if (maxSlots > 0 && filledSlots.size() > maxSlots) {
                filledSlots = List.copyOf(filledSlots.subList(0, maxSlots));
            }
            if (failChanceRemaining != null) {
                failChanceRemaining = Math.max(0.0, Math.min(1.0, failChanceRemaining));
            }
        }

        public static RenderState empty() {
            return new RenderState(0, List.of(), false, null);
        }

        public int filledCount() {
            return filledSlots.size();
        }

        public boolean isSlotFilled(int index) {
            return inscriptionAt(index) != null;
        }

        public String inscriptionAt(int index) {
            if (index < 0 || index >= filledSlots.size()) return null;
            String id = filledSlots.get(index);
            return id == null || id.isBlank() ? null : id;
        }

        public String failChanceLabel() {
            if (failed) return "铭文已失稳";
            if (failChanceRemaining == null) return "失败率 --";
            return String.format(Locale.ROOT, "失败率 %.0f%%", failChanceRemaining * 100);
        }
    }
}
