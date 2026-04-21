package com.bong.client.combat.screen;

import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonObject;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.SliderWidget;
import net.minecraft.text.Text;

/**
 * 阵法布置 UI (plan §U5 / §1 ZhenfaLayout). Lets the player pick trigger type
 * and how much qi to invest; commits via {@code combat.zhenfa_place}.
 */
public final class ZhenfaLayoutScreen extends Screen {
    public static final int BG_COLOR = 0xC0101830;
    public static final int TITLE_COLOR = 0xFF80B0FF;
    public static final int TEXT_COLOR = 0xFFD0D0D0;

    private String trigger = "proximity"; // proximity | contact | timed
    private double qiInvest = 0.5;

    public ZhenfaLayoutScreen() {
        super(Text.literal("\u9635\u6cd5\u5e03\u7f6e"));
    }

    @Override public boolean shouldPause() { return true; }

    @Override
    protected void init() {
        super.init();
        int cx = width / 2;
        int y = height / 2 - 60;
        this.addDrawableChild(triggerButton(cx - 170, y, "\u8fd1\u63a5", "proximity"));
        this.addDrawableChild(triggerButton(cx - 60, y, "\u89e6\u6c14", "contact"));
        this.addDrawableChild(triggerButton(cx + 50, y, "\u5b9a\u65f6", "timed"));

        this.addDrawableChild(new QiSlider(cx - 110, y + 40, 220, 20, qiInvest));

        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u843d\u5b9a"),
            b -> {
                JsonObject p = new JsonObject();
                p.addProperty("trigger", trigger);
                p.addProperty("qi_invest", qiInvest);
                ClientRequestSender.send("combat.zhenfa_place", p);
                this.close();
            }
        ).dimensions(cx - 50, y + 80, 100, 20).build());
    }

    private ButtonWidget triggerButton(int x, int y, String label, String key) {
        return ButtonWidget.builder(
            Text.literal(label),
            b -> {
                trigger = key;
                b.setMessage(Text.literal("\u5df2\u9009 " + label));
            }
        ).dimensions(x, y, 100, 20).build();
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        context.drawCenteredTextWithShadow(this.textRenderer, "\u9635\u6cd5\u5e03\u7f6e", width / 2, height / 2 - 90, TITLE_COLOR);
        context.drawCenteredTextWithShadow(
            this.textRenderer,
            "\u89e6\u53d1\u7c7b\u578b: " + trigger + "    \u771f\u5143: " + Math.round(qiInvest * 100) + "%",
            width / 2, height / 2 - 20, TEXT_COLOR
        );
        super.render(context, mouseX, mouseY, delta);
    }

    private final class QiSlider extends SliderWidget {
        QiSlider(int x, int y, int w, int h, double initial) {
            super(x, y, w, h, Text.literal("\u771f\u5143"), initial);
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
