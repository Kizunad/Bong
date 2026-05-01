package com.bong.client.social;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;

/** plan-social-v1 §6.1: anonymous sparring invite prompt with a 10s timeout. */
public final class SparringInviteScreen extends Screen {
    private static final int BG_COLOR = 0xD0101218;
    private static final int PANEL_COLOR = 0xE0222630;
    private static final int TITLE_COLOR = 0xFFE9D9A6;
    private static final int TEXT_COLOR = 0xFFE8E8E8;
    private static final int MUTED_COLOR = 0xFF9AA4B2;
    private static final int WARNING_COLOR = 0xFFFFAA55;

    private final SocialStateStore.SparringInvite invite;
    private boolean settled;

    public SparringInviteScreen(SocialStateStore.SparringInvite invite) {
        super(Text.literal("切磋邀请"));
        this.invite = invite;
    }

    @Override
    protected void init() {
        super.init();
        int cx = width / 2;
        int y = height / 2 + 58;
        this.addDrawableChild(ButtonWidget.builder(Text.literal("应战"), b -> settle(true, false))
            .dimensions(cx - 108, y, 96, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(Text.literal("拒绝"), b -> settle(false, false))
            .dimensions(cx + 12, y, 96, 20)
            .build());
    }

    @Override
    public void tick() {
        super.tick();
        if (!settled && remainingMillis() <= 0L) {
            settle(false, true);
        }
    }

    @Override
    public void close() {
        if (!settled) {
            settle(false, false);
            return;
        }
        super.close();
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        int panelW = Math.min(340, Math.max(280, width - 40));
        int panelH = 170;
        int panelX = (width - panelW) / 2;
        int panelY = (height - panelH) / 2;
        context.fill(panelX, panelY, panelX + panelW, panelY + panelH, PANEL_COLOR);
        context.drawCenteredTextWithShadow(textRenderer, "◇ 切 磋 邀 请 ◇", width / 2, panelY + 14, TITLE_COLOR);

        List<String> lines = describe(invite, remainingMillis()).lines();
        int y = panelY + 40;
        for (String line : lines) {
            int color = line.startsWith("倒计时") ? WARNING_COLOR : (line.startsWith("条款") ? TEXT_COLOR : MUTED_COLOR);
            context.drawCenteredTextWithShadow(textRenderer, line, width / 2, y, color);
            y += 14;
        }
        super.render(context, mouseX, mouseY, delta);
    }

    private void settle(boolean accepted, boolean timedOut) {
        if (settled) return;
        settled = true;
        ClientRequestSender.sendSparringInviteResponse(invite.inviteId(), accepted, timedOut);
        SocialStateStore.clearSparringInvite(invite.inviteId());
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.currentScreen == this) {
            mc.setScreen(null);
        }
    }

    private long remainingMillis() {
        return Math.max(0L, invite.expiresAtMs() - System.currentTimeMillis());
    }

    public String inviteIdForTests() {
        return invite.inviteId();
    }

    public static RenderContent describe(SocialStateStore.SparringInvite invite, long remainingMillis) {
        ArrayList<String> lines = new ArrayList<>();
        lines.add("发起者气息: " + fallback(invite.breathHint(), "气息相试"));
        lines.add("境界段: " + fallback(invite.realmBand(), "unknown"));
        lines.add("条款: " + fallback(invite.terms(), "无代价试炼"));
        lines.add("倒计时: " + Math.max(0L, remainingMillis / 1000L) + "s");
        lines.add("失败方: 5min 谦抑, 真元回复 -30%");
        return new RenderContent(lines);
    }

    private static String fallback(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value;
    }

    public record RenderContent(List<String> lines) {
        public RenderContent {
            lines = List.copyOf(lines);
        }
    }
}
