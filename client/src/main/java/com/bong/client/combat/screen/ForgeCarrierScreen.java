package com.bong.client.combat.screen;

import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonObject;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.SliderWidget;
import net.minecraft.text.Text;

/**
 * 暗器制作面板 (plan §U5 / §1 ForgeWeaponCarrier). Local-only UI; commits by
 * sending a {@code combat.forge_carrier_begin} C2S with selected item + qi
 * invest ratio.
 */
public final class ForgeCarrierScreen extends Screen {
    public static final int BG_COLOR = 0xC0101018;
    public static final int TITLE_COLOR = 0xFFFFE080;
    public static final int TEXT_COLOR = 0xFFD0D0D0;

    private double qiInvest = 0.5;
    private String selectedItem = "dagger"; // placeholder enum; real UI would scan inventory

    public ForgeCarrierScreen() {
        super(Text.literal("\u6697\u5668\u5236\u4f5c"));
    }

    @Override
    public boolean shouldPause() { return true; }

    @Override
    protected void init() {
        super.init();
        int centerX = width / 2;
        int panelY = height / 2 - 60;

        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u9009\u62e9: \u98de\u5200"),
            b -> { selectedItem = "dagger"; b.setMessage(Text.literal("\u5df2\u9009: \u98de\u5200")); }
        ).dimensions(centerX - 110, panelY, 100, 20).build());
        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u9009\u62e9: \u98de\u9488"),
            b -> { selectedItem = "needle"; b.setMessage(Text.literal("\u5df2\u9009: \u98de\u9488")); }
        ).dimensions(centerX + 10, panelY, 100, 20).build());

        this.addDrawableChild(new QiSlider(centerX - 110, panelY + 40, 220, 20, qiInvest));

        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u5f00\u59cb\u6ce8\u5165"),
            b -> {
                JsonObject p = new JsonObject();
                p.addProperty("item", selectedItem);
                p.addProperty("qi_invest", qiInvest);
                ClientRequestSender.send("combat.forge_carrier_begin", p);
                this.close();
            }
        ).dimensions(centerX - 50, panelY + 80, 100, 20).build());
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        context.drawCenteredTextWithShadow(
            this.textRenderer, "\u6697\u5668\u5236\u4f5c", width / 2, height / 2 - 90, TITLE_COLOR
        );
        context.drawCenteredTextWithShadow(
            this.textRenderer,
            "\u6ce8\u5165\u771f\u5143\u6bd4\u4f8b: " + Math.round(qiInvest * 100) + "%",
            width / 2, height / 2 - 20, TEXT_COLOR
        );
        super.render(context, mouseX, mouseY, delta);
    }

    private final class QiSlider extends SliderWidget {
        QiSlider(int x, int y, int w, int h, double initial) {
            super(x, y, w, h, Text.literal("\u771f\u5143\u6ce8\u5165"), initial);
            updateMessage();
        }

        @Override protected void updateMessage() {
            setMessage(Text.literal("\u771f\u5143 " + Math.round(value * 100) + "%"));
        }

        @Override protected void applyValue() {
            qiInvest = Math.max(0.0, Math.min(1.0, value));
        }
    }
}
