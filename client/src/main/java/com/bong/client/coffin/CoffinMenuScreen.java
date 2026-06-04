package com.bong.client.coffin;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;

import java.util.Objects;

/**
 * 延寿棺 G 菜单（plan-coffin-tiers-v1 P3）。
 *
 * <p>玩家瞄准延寿棺 marker 实体并按 G 键（或右键）时弹出，提供：
 * <ul>
 *   <li>[入眠] — 发送 {@code coffin_enter}，进入卧棺状态；</li>
 *   <li>[回收] — 发送 {@code coffin_menu_reclaim}，拆棺返还合成材料；</li>
 * </ul>
 * 仿 {@link com.bong.client.cultivation.voidaction.VoidActionScreen} 的极简布局：
 * 半透明背景 + 居中面板 + ButtonWidget 操作按钮。不暂停游戏（{@code shouldPause=false}）。</p>
 */
public final class CoffinMenuScreen extends Screen {
    private static final int BG_COLOR      = 0xD0101018;
    private static final int PANEL_COLOR   = 0xE0181C24;
    private static final int TITLE_COLOR   = 0xFFE9D9A6;

    /** 目标棺的任意一格坐标（server 端 registry 按 lower/upper 归一）。 */
    private final BlockPos coffinPos;

    public CoffinMenuScreen(BlockPos coffinPos) {
        super(Text.literal("延寿棺"));
        this.coffinPos = Objects.requireNonNull(coffinPos, "coffinPos must not be null");
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    protected void init() {
        super.init();
        int cx = width / 2;
        int top = height / 2 - 20;
        this.addDrawableChild(ButtonWidget.builder(
                Text.literal("【入眠】"),
                b -> onEnter())
            .dimensions(cx - 74, top, 148, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(
                Text.literal("【回收】"),
                b -> onReclaim())
            .dimensions(cx - 74, top + 26, 148, 20)
            .build());
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        int panelW = Math.min(240, Math.max(180, width - 40));
        int panelH = 80;
        int panelX = (width - panelW) / 2;
        int panelY = (height - panelH) / 2;
        context.fill(panelX, panelY, panelX + panelW, panelY + panelH, PANEL_COLOR);
        context.drawCenteredTextWithShadow(
            textRenderer,
            "◇ 延 寿 棺 ◇",
            width / 2,
            panelY + 10,
            TITLE_COLOR
        );
        super.render(context, mouseX, mouseY, delta);
    }

    private void onEnter() {
        ClientRequestSender.sendCoffinEnter(coffinPos);
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.currentScreen == this) {
            mc.setScreen(null);
        }
    }

    private void onReclaim() {
        ClientRequestSender.sendCoffinMenuReclaim(coffinPos);
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.currentScreen == this) {
            mc.setScreen(null);
        }
    }
}
