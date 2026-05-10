package com.bong.client.npc;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.List;

public final class NpcTradeScreen extends Screen {
    private final NpcMetadata metadata;

    public NpcTradeScreen(NpcMetadata metadata) {
        super(Text.literal(metadata == null ? "NPC Trade" : metadata.displayName()));
        this.metadata = metadata;
    }

    @Override
    protected void init() {
        if (metadata == null) {
            close();
            return;
        }
        int centerX = width / 2;
        ButtonWidget buyHerb = ButtonWidget.builder(Text.literal("灵草"), button -> {
                ClientRequestSender.sendNpcTradeRequest(metadata.entityId(), List.of(), "lingcao");
                close();
            })
            .dimensions(centerX - 96, height / 2 + 26, 58, 20)
            .build();
        buyHerb.active = metadata.tradeCandidate();
        addDrawableChild(buyHerb);
        addDrawableChild(ButtonWidget.builder(Text.literal("残卷"), button -> {
                ClientRequestSender.sendNpcTradeRequest(metadata.entityId(), List.of(), "fragment_scroll");
                close();
            })
            .dimensions(centerX - 29, height / 2 + 26, 58, 20)
            .build()).active = metadata.tradeCandidate();
        addDrawableChild(ButtonWidget.builder(Text.literal("返回"), button ->
                client.setScreen(new NpcDialogueScreen(metadata)))
            .dimensions(centerX + 38, height / 2 + 26, 58, 20)
            .build());
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        renderBackground(context);
        if (metadata != null) {
            int centerX = width / 2;
            context.drawCenteredTextWithShadow(textRenderer, metadata.displayName(), centerX, height / 2 - 66, 0xE8D8A8);
            context.drawTextWithShadow(textRenderer, "NPC 出售", centerX - 112, height / 2 - 32, 0xD8D8D8);
            context.drawTextWithShadow(textRenderer, "玩家出价", centerX + 34, height / 2 - 32, 0xD8D8D8);
            String price = metadata.reputationToPlayer() > 50 ? "骨币 ×0.8" : "骨币 标价";
            context.drawTextWithShadow(textRenderer, price, centerX - 112, height / 2 - 12, 0xB8E6B8);
            if (metadata.hostile()) {
                context.drawCenteredTextWithShadow(textRenderer, "此人对你充满敌意", centerX, height / 2 + 4, 0xE05A47);
            }
        }
        super.render(context, mouseX, mouseY, delta);
    }
}
