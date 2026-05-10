package com.bong.client.npc;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

public final class NpcDialogueScreen extends Screen {
    private final NpcMetadata metadata;

    public NpcDialogueScreen(NpcMetadata metadata) {
        super(Text.literal(metadata == null ? "NPC" : metadata.displayName()));
        this.metadata = metadata;
    }

    @Override
    protected void init() {
        if (metadata == null) {
            close();
            return;
        }
        int centerX = width / 2;
        int y = height / 2 - 10;
        addDrawableChild(ButtonWidget.builder(Text.literal("查看"), button -> {
                ClientRequestSender.sendNpcDialogueChoice(metadata.entityId(), "inspect");
                ClientRequestSender.sendNpcInspectRequest(metadata.entityId());
                client.setScreen(new NpcInspectScreen(metadata));
            })
            .dimensions(centerX - 94, y, 58, 20)
            .build());
        addDrawableChild(ButtonWidget.builder(Text.literal("交易"), button -> {
                ClientRequestSender.sendNpcDialogueChoice(metadata.entityId(), "trade");
                client.setScreen(new NpcTradeScreen(metadata));
            })
            .dimensions(centerX - 29, y, 58, 20)
            .build()).active = metadata.tradeCandidate();
        addDrawableChild(ButtonWidget.builder(Text.literal("离开"), button -> {
                ClientRequestSender.sendNpcDialogueChoice(metadata.entityId(), "leave");
                close();
            })
            .dimensions(centerX + 36, y, 58, 20)
            .build());
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        renderBackground(context);
        if (metadata != null) {
            int centerX = width / 2;
            context.drawCenteredTextWithShadow(textRenderer, metadata.displayName(), centerX, height / 2 - 60, 0xE8D8A8);
            context.drawCenteredTextWithShadow(textRenderer, greetingText(metadata), centerX, height / 2 - 38, 0xD0D0D0);
        }
        super.render(context, mouseX, mouseY, delta);
    }

    private static String greetingText(NpcMetadata metadata) {
        return metadata.hostile() ? "此人对你充满敌意。" : metadata.greetingText();
    }
}
