package com.bong.client.hud;

import com.bong.client.state.NarrationState;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;

import java.util.Objects;
import java.util.Optional;

public final class BongToast {
    static final int WARNING_COLOR = 0xFFAA55;
    static final int ERA_DECREE_COLOR = 0xFFE080;
    private static final int BACKGROUND_COLOR = 0x88000000;
    private static final int HORIZONTAL_PADDING = 4;
    private static final int VERTICAL_PADDING = 4;

    private static volatile BongToast activeToast = empty();

    private final Text text;
    private final int color;
    private final long shownAtMillis;
    private final long expiresAtMillis;

    private BongToast(Text text, int color, long shownAtMillis, long expiresAtMillis) {
        this.text = Objects.requireNonNull(text, "text");
        this.color = color;
        this.shownAtMillis = Math.max(0L, shownAtMillis);
        this.expiresAtMillis = Math.max(0L, expiresAtMillis);
    }

    public static BongToast empty() {
        return new BongToast(Text.empty(), 0xFFFFFF, 0L, 0L);
    }

    public static BongToast create(NarrationState narrationState, long shownAtMillis) {
        if (narrationState == null || narrationState.isEmpty() || !narrationState.isToastEligible()) {
            return empty();
        }

        return new BongToast(
            toastText(narrationState),
            toastColor(narrationState),
            Math.max(0L, shownAtMillis),
            Math.max(0L, shownAtMillis) + narrationState.toastDurationMillis()
        );
    }

    public static BongToast create(String text, int color, long shownAtMillis, long durationMillis) {
        String normalizedText = text == null ? "" : text.trim();
        long normalizedDurationMillis = Math.max(0L, durationMillis);
        if (normalizedText.isEmpty() || normalizedDurationMillis == 0L) {
            return empty();
        }

        return new BongToast(
            Text.literal(normalizedText),
            color,
            Math.max(0L, shownAtMillis),
            Math.max(0L, shownAtMillis) + normalizedDurationMillis
        );
    }

    public static void show(NarrationState narrationState, long shownAtMillis) {
        BongToast candidate = create(narrationState, shownAtMillis);
        if (!candidate.isEmpty()) {
            activeToast = candidate;
        }
    }

    public static void show(String text, int color, long shownAtMillis, long durationMillis) {
        BongToast candidate = create(text, color, shownAtMillis, durationMillis);
        if (!candidate.isEmpty()) {
            activeToast = candidate;
        }
    }

    public static BongToast current(long nowMillis) {
        BongToast snapshot = activeToast;
        if (!snapshot.isActiveAt(nowMillis)) {
            activeToast = empty();
            return empty();
        }
        return snapshot;
    }

    public static Optional<HudRenderCommand> buildCommand(long nowMillis, HudTextHelper.WidthMeasurer widthMeasurer, int maxWidth) {
        BongToast toast = current(nowMillis);
        if (toast.isEmpty()) {
            return Optional.empty();
        }

        String clippedText = HudTextHelper.clipToWidth(toast.text().getString(), maxWidth, widthMeasurer);
        if (clippedText.isEmpty()) {
            return Optional.empty();
        }

        int xOffset = HudAnimation.toastSlideOffset(toast.shownAtMillis(), toast.expiresAtMillis(), nowMillis, 28);
        return Optional.of(HudRenderCommand.toast(HudRenderLayer.TOAST, clippedText, xOffset, 0, toast.color()));
    }

    public static void render(
        DrawContext context,
        TextRenderer textRenderer,
        int scaledWidth,
        int scaledHeight,
        HudRenderCommand command
    ) {
        if (context == null || textRenderer == null || command == null || !command.isToast()) {
            return;
        }

        String message = command.text();
        if (message == null || message.isBlank()) {
            return;
        }

        int width = textRenderer.getWidth(message);
        int x = Math.max(0, (scaledWidth - width) / 2 + command.x());
        int y = Math.max(0, scaledHeight / 4);
        context.fill(
            x - HORIZONTAL_PADDING,
            y - VERTICAL_PADDING,
            x + width + HORIZONTAL_PADDING,
            y + textRenderer.fontHeight + VERTICAL_PADDING,
            BACKGROUND_COLOR
        );
        context.drawTextWithShadow(textRenderer, message, x, y, command.color());
    }

    public Text text() {
        return text;
    }

    public int color() {
        return color;
    }

    public long shownAtMillis() {
        return shownAtMillis;
    }

    public long expiresAtMillis() {
        return expiresAtMillis;
    }

    public boolean isEmpty() {
        return text.getString().isBlank() || expiresAtMillis <= 0L;
    }

    public boolean isActiveAt(long nowMillis) {
        return !isEmpty() && Math.max(0L, nowMillis) < expiresAtMillis;
    }

    static void resetForTests() {
        activeToast = empty();
    }

    private static Text toastText(NarrationState narrationState) {
        return switch (narrationState.style()) {
            case SYSTEM_WARNING -> Text.literal("天道警示：").formatted(Formatting.RED, Formatting.BOLD)
                .append(Text.literal(narrationState.text()).formatted(Formatting.RED));
            case ERA_DECREE -> Text.literal("时代法旨：").formatted(Formatting.GOLD, Formatting.BOLD)
                .append(Text.literal(narrationState.text()).formatted(Formatting.GOLD));
            default -> Text.literal(narrationState.text());
        };
    }

    private static int toastColor(NarrationState narrationState) {
        return switch (narrationState.style()) {
            case SYSTEM_WARNING -> WARNING_COLOR;
            case ERA_DECREE -> ERA_DECREE_COLOR;
            default -> 0xFFFFFF;
        };
    }
}
