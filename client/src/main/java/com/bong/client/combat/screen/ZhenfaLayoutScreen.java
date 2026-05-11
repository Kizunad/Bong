package com.bong.client.combat.screen;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.network.ClientRequestProtocol;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.SliderWidget;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;

/**
 * 阵法布置 UI (plan §U5 / §1 ZhenfaLayout). Classic arrays let the player
 * pick trigger type; ordinary traps use fixed trigger semantics.
 */
public final class ZhenfaLayoutScreen extends Screen {
    public static final int BG_COLOR = 0xC0101830;
    public static final int TITLE_COLOR = 0xFF80B0FF;
    public static final int TEXT_COLOR = 0xFFD0D0D0;

    private String trigger = "proximity"; // proximity | contact | timed
    private double qiInvest = 0.1;
    private final BlockPos targetPos;
    private final ClientRequestProtocol.ZhenfaKind kind;
    private final long itemInstanceId;
    private final ClientRequestProtocol.ZhenfaTargetFace targetFace;

    public ZhenfaLayoutScreen() {
        this(new BlockPos(0, 64, 0));
    }

    public ZhenfaLayoutScreen(BlockPos targetPos) {
        this(targetPos, ClientRequestProtocol.ZhenfaKind.TRAP, 0L, null);
    }

    public ZhenfaLayoutScreen(
        BlockPos targetPos,
        ClientRequestProtocol.ZhenfaKind kind,
        long itemInstanceId,
        ClientRequestProtocol.ZhenfaTargetFace targetFace
    ) {
        super(Text.literal("\u9635\u6cd5\u5e03\u7f6e"));
        this.targetPos = targetPos == null ? new BlockPos(0, 64, 0) : targetPos;
        this.kind = kind == null ? ClientRequestProtocol.ZhenfaKind.TRAP : kind;
        this.itemInstanceId = itemInstanceId;
        this.targetFace = targetFace;
    }

    @Override public boolean shouldPause() { return true; }

    @Override
    protected void init() {
        super.init();
        int cx = width / 2;
        int y = height / 2 - 60;
        if (!usesFixedTrapTrigger()) {
            this.addDrawableChild(triggerButton(cx - 170, y, "\u8fd1\u63a5", "proximity"));
            this.addDrawableChild(triggerButton(cx - 60, y, "\u89e6\u6c14", "contact"));
            this.addDrawableChild(triggerButton(cx + 50, y, "\u5b9a\u65f6", "timed"));
        }

        this.addDrawableChild(new QiSlider(cx - 110, y + 40, 220, 20, qiInvest));

        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u843d\u5b9a"),
            b -> {
                ClientRequestSender.sendZhenfaPlace(
                    targetPos,
                    kind,
                    ClientRequestProtocol.ZhenfaCarrierKind.COMMON_STONE,
                    qiInvest,
                    usesFixedTrapTrigger() ? null : trigger,
                    itemInstanceId > 0 ? itemInstanceId : null,
                    targetFace
                );
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

    private boolean usesFixedTrapTrigger() {
        return kind == ClientRequestProtocol.ZhenfaKind.WARNING_TRAP
            || kind == ClientRequestProtocol.ZhenfaKind.BLAST_TRAP
            || kind == ClientRequestProtocol.ZhenfaKind.SLOW_TRAP;
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        context.drawCenteredTextWithShadow(this.textRenderer, "\u9635\u6cd5\u5e03\u7f6e", width / 2, height / 2 - 90, TITLE_COLOR);
        context.drawCenteredTextWithShadow(
            this.textRenderer,
            (usesFixedTrapTrigger() ? "\u89e6\u53d1\u7c7b\u578b: \u56fa\u5b9a" : "\u89e6\u53d1\u7c7b\u578b: " + trigger)
                + "    \u771f\u5143: "
                + Math.round(qiInvest * 100)
                + "%",
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
