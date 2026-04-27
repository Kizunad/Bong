package com.bong.client.combat.screen;

import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

/**
 * Final termination overlay (plan §U4). Displays final words + epilogue + a
 * "create new character" button.
 */
public final class TerminateScreen extends Screen {
    public static final int BG_COLOR = 0xF0000000;
    public static final int TITLE_COLOR = 0xFFBB66FF;
    public static final int TEXT_COLOR = 0xFFD0D0D0;

    private final TerminateStateStore.State state;

    public TerminateScreen(TerminateStateStore.State state) {
        super(Text.literal("\u7ec8\u7ed3"));
        this.state = state == null ? TerminateStateStore.State.HIDDEN : state;
    }

    @Override
    public boolean shouldPause() { return true; }

    @Override
    public boolean shouldCloseOnEsc() { return false; }

    @Override
    protected void init() {
        super.init();
        int y = height - 60;
        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u521b\u5efa\u65b0\u89d2\u8272"),
            b -> ClientRequestSender.send("combat_create_new_character", null)
        ).dimensions(width / 2 - 80, y, 160, 20).build());
    }

    @Override
    public void tick() {
        super.tick();
        if (!TerminateStateStore.snapshot().visible()) {
            this.close();
        }
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        context.drawCenteredTextWithShadow(
            this.textRenderer, "\u2015\u2015 \u7ec8\u7109\u4e4b\u8a00 \u2015\u2015", width / 2, 60, TITLE_COLOR
        );
        int y = 86;
        for (String line : state.finalWords().split("\\r?\\n")) {
            if (line.isBlank()) continue;
            context.drawCenteredTextWithShadow(this.textRenderer, line, width / 2, y, TEXT_COLOR);
            y += 14;
        }
        y += 18;
        if (!state.epilogue().isBlank()) {
            context.drawCenteredTextWithShadow(
                this.textRenderer, state.epilogue(), width / 2, y, TEXT_COLOR
            );
            y += 14;
        }
        if (!state.archetypeSuggestion().isBlank()) {
            context.drawCenteredTextWithShadow(
                this.textRenderer,
                "\u5efa\u8bae\u65b0\u89d2\u8272\u539f\u578b: " + state.archetypeSuggestion(),
                width / 2, y + 24, TITLE_COLOR
            );
        }
        super.render(context, mouseX, mouseY, delta);
    }

    TerminateStateStore.State stateForTests() { return state; }
}
