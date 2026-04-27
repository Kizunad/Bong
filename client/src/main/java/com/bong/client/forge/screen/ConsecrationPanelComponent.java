package com.bong.client.forge.screen;

import com.bong.client.cultivation.ColorKind;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
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

import java.util.List;
import java.util.Locale;

/** plan-forge-leftovers-v1 §5.1 — 开光真元注入条。 */
public class ConsecrationPanelComponent extends BaseComponent {
    public static final int PANEL_WIDTH = 260;
    public static final int PANEL_HEIGHT = 104;
    public static final double QI_PER_TICK = 2.5;

    private static final int BG_COLOR = 0xCC10171A;
    private static final int BORDER_COLOR = 0xFF35545E;
    private static final int TEXT_COLOR = 0xFFD9F4F4;
    private static final int MUTED_TEXT_COLOR = 0xFF89A2A8;
    private static final int BAR_BG = 0xFF1D2A30;
    private static final int BAR_FILL = 0xFF55D4D8;
    private static final int BUTTON_BG = 0xFF24464B;
    private static final int BUTTON_HOVER = 0xFF2F6068;
    private static final int BUTTON_DISABLED = 0xFF342124;
    private static final int WARNING_COLOR = 0xFFFF7777;

    private boolean injecting;
    private long lastInjectTick = -1L;

    public ConsecrationPanelComponent() {
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
        return renderStateFrom(ForgeSessionStore.snapshot(), InventoryStateStore.snapshot().realm());
    }

    public static RenderState renderStateFrom(ForgeSessionStore.Snapshot snapshot, String casterRealm) {
        if (snapshot == null || !"consecration".equals(snapshot.currentStep())) {
            return RenderState.empty(casterRealm);
        }

        JsonObject state = parseJsonObject(snapshot.stepStateJson());
        double injected = readNonNegativeDouble(state, "qi_injected", 0.0);
        double required = readNonNegativeDouble(state, "qi_required", 0.0);
        ColorKind color = ColorKind.fromWire(readOptionalString(state, "color_imprint"));
        String minRealm = readOptionalString(state, "min_realm");
        return new RenderState(injected, required, color, casterRealm == null ? "" : casterRealm, minRealm);
    }

    public boolean beginInject() {
        RenderState state = currentRenderState();
        if (!state.canInject()) return false;
        injecting = true;
        lastInjectTick = -1L;
        return true;
    }

    public void endInject() {
        injecting = false;
        lastInjectTick = -1L;
    }

    public int tickInject(long gameTick) {
        if (!injecting) return 0;
        RenderState state = currentRenderState();
        if (!state.canInject()) {
            endInject();
            return 0;
        }

        long safeTick = Math.max(0L, gameTick);
        if (safeTick == lastInjectTick) return 0;
        lastInjectTick = safeTick;
        sendInject(state, QI_PER_TICK);
        return 1;
    }

    public boolean isInjecting() {
        return injecting;
    }

    public boolean isOverInjectButton(double mouseX, double mouseY) {
        return mouseX >= buttonX() && mouseX < buttonX() + 86
            && mouseY >= buttonY() && mouseY < buttonY() + 20;
    }

    public int sendInject(RenderState state, double qiAmount) {
        ForgeSessionStore.Snapshot snapshot = ForgeSessionStore.snapshot();
        if (snapshot.sessionId() <= 0 || !"consecration".equals(snapshot.currentStep())) return 0;
        RenderState safeState = state == null ? currentRenderState() : state;
        if (!safeState.canInject() || qiAmount <= 0 || !Double.isFinite(qiAmount)) return 0;
        ClientRequestSender.sendForgeConsecrationInject(snapshot.sessionId(), qiAmount);
        return 1;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        drawPanel(context, currentRenderState(), x, y, isOverInjectButton(mouseX, mouseY));
    }

    public static void drawPanel(OwoUIDrawContext context, RenderState state, int x, int y, boolean buttonHover) {
        if (context == null) return;
        RenderState safeState = state == null ? RenderState.empty("") : state;
        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;

        context.fill(x, y, x + PANEL_WIDTH, y + PANEL_HEIGHT, BG_COLOR);
        drawBorder(context, x, y, PANEL_WIDTH, PANEL_HEIGHT, BORDER_COLOR);
        context.drawTextWithShadow(textRenderer, Text.literal("开光注入"), x + 8, y + 7, TEXT_COLOR);
        context.drawTextWithShadow(textRenderer, Text.literal(safeState.qiLabel()), x + 154, y + 7, MUTED_TEXT_COLOR);

        int barX = x + 12;
        int barY = y + 36;
        int barW = PANEL_WIDTH - 24;
        int barH = 10;
        context.fill(barX, barY, barX + barW, barY + barH, BAR_BG);
        int fillW = (int) Math.round(barW * safeState.progressRatio());
        if (fillW > 0) context.fill(barX, barY, barX + fillW, barY + barH, BAR_FILL);
        drawBorder(context, barX, barY, barW, barH, 0xFF4B6870);

        drawSwatch(context, x + 18, y + 62, safeState.color());
        context.drawTextWithShadow(textRenderer, Text.literal(safeState.colorLabel()), x + 40, y + 64, MUTED_TEXT_COLOR);

        int buttonColor = safeState.canInject()
            ? (buttonHover ? BUTTON_HOVER : BUTTON_BG)
            : BUTTON_DISABLED;
        context.fill(x + 160, y + 60, x + 246, y + 80, buttonColor);
        drawBorder(context, x + 160, y + 60, 86, 20, safeState.canInject() ? BORDER_COLOR : WARNING_COLOR);
        String buttonText = safeState.isComplete() ? "已注满" : "按住注入";
        int textX = x + 160 + (86 - textRenderer.getWidth(buttonText)) / 2;
        context.drawTextWithShadow(textRenderer, Text.literal(buttonText), textX, y + 66,
            safeState.canInject() ? TEXT_COLOR : WARNING_COLOR);

        if (!safeState.canInject() && !safeState.isComplete()) {
            context.drawTextWithShadow(textRenderer, Text.literal(safeState.realmGateLabel()),
                x + 8, y + 88, WARNING_COLOR);
        }
    }

    private static void drawSwatch(OwoUIDrawContext context, int x, int y, ColorKind color) {
        int argb = color == null ? 0xFF4B5960 : color.argb();
        context.fill(x + 3, y, x + 13, y + 16, argb);
        context.fill(x, y + 3, x + 16, y + 13, argb);
        drawBorder(context, x, y, 16, 16, 0xFF2A3338);
    }

    private int buttonX() {
        return x + 160;
    }

    private int buttonY() {
        return y + 60;
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

    private static double readNonNegativeDouble(JsonObject state, String key, double fallback) {
        if (state == null || !state.has(key)) return Math.max(0.0, fallback);
        try {
            double value = state.get(key).getAsDouble();
            return Double.isFinite(value) ? Math.max(0.0, value) : Math.max(0.0, fallback);
        } catch (RuntimeException ignored) {
            return Math.max(0.0, fallback);
        }
    }

    private static String readOptionalString(JsonObject state, String key) {
        if (state == null || !state.has(key) || state.get(key).isJsonNull()) return "";
        try {
            return state.get(key).getAsString().trim();
        } catch (RuntimeException ignored) {
            return "";
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

    public record RenderState(
        double qiInjected,
        double qiRequired,
        ColorKind color,
        String casterRealm,
        String minRealm
    ) {
        public RenderState {
            qiInjected = finiteNonNegative(qiInjected);
            qiRequired = finiteNonNegative(qiRequired);
            casterRealm = casterRealm == null ? "" : casterRealm.trim();
            minRealm = minRealm == null ? "" : minRealm.trim();
        }

        public static RenderState empty(String casterRealm) {
            return new RenderState(0.0, 0.0, null, casterRealm, "");
        }

        public double progressRatio() {
            if (qiRequired <= 0.0) return 0.0;
            return Math.max(0.0, Math.min(1.0, qiInjected / qiRequired));
        }

        public boolean isComplete() {
            return qiRequired > 0.0 && qiInjected >= qiRequired;
        }

        public boolean realmAllowed() {
            if (minRealm.isBlank()) return true;
            int min = realmRank(minRealm);
            int caster = realmRank(casterRealm);
            return min < 0 || caster >= min;
        }

        public boolean canInject() {
            return qiRequired > 0.0 && !isComplete() && realmAllowed();
        }

        public String colorLabel() {
            return color == null ? "真元色未定" : "真元色 " + color.label();
        }

        public String qiLabel() {
            return String.format(Locale.ROOT, "%.0f / %.0f", qiInjected, qiRequired);
        }

        public String realmGateLabel() {
            if (minRealm.isBlank()) return "境界不足";
            return "境界不足：需 " + minRealm;
        }

        private static double finiteNonNegative(double value) {
            return Double.isFinite(value) ? Math.max(0.0, value) : 0.0;
        }

        private static int realmRank(String realm) {
            if (realm == null || realm.isBlank()) return -1;
            String normalized = realm.trim();
            List<String> order = List.of("Awaken", "Induce", "Condense", "Solidify", "Spirit", "Void");
            for (int i = 0; i < order.size(); i++) {
                if (order.get(i).equalsIgnoreCase(normalized)) return i;
            }
            return -1;
        }
    }
}
