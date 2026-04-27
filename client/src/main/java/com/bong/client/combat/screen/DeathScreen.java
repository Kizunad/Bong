package com.bong.client.combat.screen;

import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.List;

/**
 * Full-screen death overlay (plan §U3 / §2.3). Shows 重生概率 / 遗念 / 60s
 * countdown + 重生/终结 buttons.
 *
 * <p>Authoritative state comes from {@link DeathStateStore}; when the server
 * hides the death screen this GUI closes itself on next tick.
 */
public final class DeathScreen extends Screen {
    public static final int BG_COLOR = 0xE0000000;
    public static final int TITLE_COLOR = 0xFFFF4040;
    public static final int TEXT_COLOR = 0xFFDDDDDD;
    public static final int LUCK_FILL_COLOR = 0xFFE0C040;
    public static final int LUCK_TRACK_COLOR = 0xFF303030;

    private final DeathStateStore.State state;
    private long lastRenderMs;

    public DeathScreen(DeathStateStore.State state) {
        super(Text.literal("死亡"));
        this.state = state == null ? DeathStateStore.State.HIDDEN : state;
    }

    @Override
    public boolean shouldPause() { return false; }

    @Override
    public boolean shouldCloseOnEsc() { return false; }

    @Override
    protected void init() {
        super.init();
        int centerX = width / 2;
        int y = height - 80;
        if (state.canReincarnate()) {
            this.addDrawableChild(ButtonWidget.builder(
                Text.literal("\u91cd\u751f"),
                b -> ClientRequestSender.send("combat_reincarnate", null)
            ).dimensions(centerX - 110, y, 100, 20).build());
        }
        if (state.canTerminate()) {
            this.addDrawableChild(ButtonWidget.builder(
                Text.literal("\u7ec8\u7ed3"),
                b -> ClientRequestSender.send("combat_terminate", null)
            ).dimensions(centerX + 10, y, 100, 20).build());
        }
    }

    @Override
    public void tick() {
        super.tick();
        // Auto-close if server has hidden the screen.
        if (!DeathStateStore.snapshot().visible()) {
            this.close();
        }
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        long now = System.currentTimeMillis();
        this.lastRenderMs = now;
        context.fill(0, 0, width, height, BG_COLOR);

        String title = "\u9053\u9668\u2015\u2015" + causeLabel(state.cause());
        context.drawCenteredTextWithShadow(this.textRenderer, title, width / 2, 30, TITLE_COLOR);

        // Luck
        int luckBarW = 220;
        int luckBarX = (width - luckBarW) / 2;
        int luckBarY = 64;
        context.fill(luckBarX, luckBarY, luckBarX + luckBarW, luckBarY + 6, LUCK_TRACK_COLOR);
        int fill = Math.round(state.luckRemaining() * luckBarW);
        if (fill > 0) {
            context.fill(luckBarX, luckBarY, luckBarX + fill, luckBarY + 6, LUCK_FILL_COLOR);
        }
        context.drawCenteredTextWithShadow(
            this.textRenderer,
            "\u91cd\u751f\u6982\u7387: " + Math.round(state.luckRemaining() * 100) + "%",
            width / 2, luckBarY - 12, TEXT_COLOR
        );

        String phase = phaseLabel(state.stage());
        String zone = zoneLabel(state.zoneKind());
        String deathNo = state.deathNumber() > 0 ? " \u00b7 \u7b2c" + state.deathNumber() + "\u6b7b" : "";
        context.drawCenteredTextWithShadow(
            this.textRenderer,
            phase + deathNo + (zone.isEmpty() ? "" : " \u00b7 " + zone),
            width / 2, luckBarY + 9, TEXT_COLOR
        );

        // Countdown
        long rem = state.remainingMs(now);
        String countdown = "\u5012\u8ba1\u65f6: " + (rem / 1000) + "s";
        context.drawCenteredTextWithShadow(
            this.textRenderer, countdown, width / 2, luckBarY + 32, TITLE_COLOR
        );

        if (state.hasLifespanPreview()) {
            String lifespan = String.format(
                "\u5bff\u5143 %.1f/%d \u00b7 \u4f59%.1f \u00b7 \u672c\u6b7b\u6263%d \u00b7 \u6d41\u901f\u00d7%.1f%s",
                state.yearsLived(), state.lifespanCapByRealm(), state.remainingYears(),
                state.deathPenaltyYears(), state.lifespanTickRateMultiplier(),
                state.windCandle() ? " \u00b7 \u98ce\u70db" : ""
            );
            context.drawCenteredTextWithShadow(
                this.textRenderer, lifespan, width / 2, luckBarY + 46, TEXT_COLOR
            );
        }

        // Final words
        int wy = luckBarY + (state.hasLifespanPreview() ? 74 : 60);
        context.drawCenteredTextWithShadow(
            this.textRenderer, "\u9057\u5ff5", width / 2, wy, TITLE_COLOR
        );
        int line = 0;
        for (String word : state.finalWords()) {
            if (line >= 6) break;
            context.drawCenteredTextWithShadow(
                this.textRenderer, "\u300c" + word + "\u300d", width / 2, wy + 14 + line * 12, TEXT_COLOR
            );
            line++;
        }

        super.render(context, mouseX, mouseY, delta);
    }

    private static String causeLabel(String cause) {
        return switch (cause == null ? "" : cause) {
            case "pk" -> "\u6b7b\u4e8ePK";
            case "tribulation" -> "\u6b7b\u4e8e\u5929\u52ab";
            case "dao_heart_shatter" -> "\u9053\u5fc3\u5d29\u584c";
            case "starvation" -> "\u9965\u6b7b";
            default -> cause == null || cause.isBlank() ? "\u672a\u77e5" : cause;
        };
    }

    private static String phaseLabel(String stage) {
        return switch (stage == null ? "" : stage) {
            case "fortune" -> "\u8fd0\u6570\u671f";
            case "tribulation" -> "\u52ab\u6570\u671f";
            default -> "\u91cd\u751f\u5224\u5b9a";
        };
    }

    private static String zoneLabel(String zoneKind) {
        return switch (zoneKind == null ? "" : zoneKind) {
            case "death" -> "\u6b7b\u57df\uff1a\u8df3\u8fc7\u8fd0\u6570";
            case "negative" -> "\u8d1f\u7075\u57df\uff1a\u8df3\u8fc7\u8fd0\u6570";
            default -> "";
        };
    }

    long lastRenderForTests() { return lastRenderMs; }
    DeathStateStore.State stateForTests() { return state; }
}
