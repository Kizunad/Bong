package com.bong.client.combat.inspect;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.util.MeridianGateLabel;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.List;

/** Tiny meridian silhouette summary for the techniques workspace. */
public final class MeridianMiniSilhouette extends BaseComponent {
    private static final int WIDTH = 176;
    private static final int HEIGHT = 58;
    private List<TechniquesListPanel.RequiredMeridian> required = List.of();

    public MeridianMiniSilhouette() {
        this.sizing(Sizing.fixed(WIDTH), Sizing.fixed(HEIGHT));
    }

    public void refresh(TechniquesListPanel.Technique technique) {
        this.required = technique == null ? List.of() : technique.requiredMeridians();
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        context.fill(x, y, x + WIDTH, y + HEIGHT, 0xFF101515);
        drawBorder(context, 0xFF345048);
        var tr = MinecraftClient.getInstance().textRenderer;
        context.drawTextWithShadow(tr, Text.literal("经脉需求"), x + 4, y + 4, 0xFF9FE0D0);
        context.drawTextWithShadow(
            tr,
            Text.literal(MeridianGateLabel.spiritExtraordinaryProgress(MeridianStateStore.snapshot())),
            x + 94,
            y + 4,
            0xFFDDAAFF
        );
        if (required.isEmpty()) {
            context.drawTextWithShadow(tr, Text.literal("无特定经脉要求"), x + 4, y + 20, 0xFFAAAAAA);
            return;
        }
        MeridianBody body = MeridianStateStore.snapshot();
        int cy = y + 18;
        for (int i = 0; i < Math.min(3, required.size()); i++) {
            var r = required.get(i);
            MeridianChannel channel = TechniquesListPanel.channelFromWire(r.channel()).orElse(null);
            String channelName = channel == null ? r.channel() : channel.displayName();
            String line = channelName + "  健康≥" + Math.round(r.minHealth() * 100.0f) + "%";
            context.drawTextWithShadow(tr, Text.literal(line), x + 4, cy, requirementColor(body, channel));
            cy += 12;
        }
    }

    private static int requirementColor(MeridianBody body, MeridianChannel channel) {
        if (body == null || channel == null) return 0xFFC8E8D8;
        ChannelState state = body.channel(channel);
        if (state == null) return 0xFFC8E8D8;
        if (state.damage() == ChannelState.DamageLevel.SEVERED) return 0xFF888888;
        if (state.blocked()) return 0xFFCC6666;
        return state.damage().color();
    }

    private void drawBorder(OwoUIDrawContext context, int color) {
        context.fill(x, y, x + WIDTH, y + 1, color);
        context.fill(x, y + HEIGHT - 1, x + WIDTH, y + HEIGHT, color);
        context.fill(x, y, x + 1, y + HEIGHT, color);
        context.fill(x + WIDTH - 1, y, x + WIDTH, y + HEIGHT, color);
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return WIDTH; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return HEIGHT; }
}
