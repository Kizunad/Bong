package com.bong.client.cultivation.voidaction;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

public final class VoidActionScreen extends Screen {
    private static final int BG_COLOR = 0xD0101018;
    private static final int PANEL_COLOR = 0xE0181C24;
    private static final int TITLE_COLOR = 0xFFE9D9A6;
    private static final int TEXT_COLOR = 0xFFE8E8E8;
    private static final int MUTED_COLOR = 0xFF9AA4B2;
    private static final int WARNING_COLOR = 0xFFFFAA55;

    public VoidActionScreen() {
        super(Text.literal("化虚行事"));
    }

    @Override
    protected void init() {
        super.init();
        int cx = width / 2;
        int top = height / 2 - 20;
        addActionButton(VoidActionKind.SUPPRESS_TSY, cx - 154, top);
        addActionButton(VoidActionKind.EXPLODE_ZONE, cx + 6, top);
        addActionButton(VoidActionKind.BARRIER, cx - 154, top + 28);
        this.addDrawableChild(ButtonWidget.builder(
                Text.literal(VoidActionKind.LEGACY_ASSIGN.label()),
                b -> openLegacyPanel()
            )
            .dimensions(cx + 6, top + 28, 148, 20)
            .build());
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        int panelW = Math.min(380, Math.max(320, width - 40));
        int panelH = 188;
        int panelX = (width - panelW) / 2;
        int panelY = (height - panelH) / 2;
        VoidActionStore.Snapshot snapshot = VoidActionStore.snapshot();
        long nowTick = nowTick();

        context.fill(panelX, panelY, panelX + panelW, panelY + panelH, PANEL_COLOR);
        context.drawCenteredTextWithShadow(textRenderer, "◇ 化 虚 行 事 ◇", width / 2, panelY + 14, TITLE_COLOR);
        context.drawCenteredTextWithShadow(textRenderer, "目标: " + snapshot.targetZoneId(), width / 2, panelY + 34, TEXT_COLOR);

        int y = panelY + 102;
        for (VoidActionKind kind : VoidActionKind.values()) {
            context.drawTextWithShadow(
                textRenderer,
                describe(kind, snapshot, nowTick),
                panelX + 24,
                y,
                snapshot.ready(kind, nowTick) ? MUTED_COLOR : WARNING_COLOR
            );
            y += 14;
        }
        super.render(context, mouseX, mouseY, delta);
    }

    private void addActionButton(VoidActionKind kind, int x, int y) {
        this.addDrawableChild(ButtonWidget.builder(Text.literal(kind.label()), b -> dispatch(kind))
            .dimensions(x, y, 148, 20)
            .build());
    }

    private void dispatch(VoidActionKind kind) {
        MinecraftClient mc = MinecraftClient.getInstance();
        VoidActionStore.Snapshot snapshot = VoidActionStore.snapshot();
        long nowTick = nowTick();
        boolean sent = switch (kind) {
            case SUPPRESS_TSY -> VoidActionHandler.dispatchSuppressTsy(snapshot.targetZoneId(), nowTick);
            case EXPLODE_ZONE -> VoidActionHandler.dispatchExplodeZone(snapshot.targetZoneId(), nowTick);
            case BARRIER -> {
                double x = mc.player == null ? 0.0 : mc.player.getX();
                double y = mc.player == null ? 64.0 : mc.player.getY();
                double z = mc.player == null ? 0.0 : mc.player.getZ();
                yield VoidActionHandler.dispatchBarrier(snapshot.targetZoneId(), x, y, z, nowTick);
            }
            case LEGACY_ASSIGN -> false;
        };
        if (sent && mc.currentScreen == this) {
            mc.setScreen(null);
        }
    }

    private void openLegacyPanel() {
        MinecraftClient.getInstance().setScreen(new LegacyAssignPanel());
    }

    private static String describe(VoidActionKind kind, VoidActionStore.Snapshot snapshot, long nowTick) {
        String cost = kind.qiCost() <= 0.0
            ? "无即时真元 / 寿元代价"
            : "真元 " + Math.round(kind.qiCost()) + " · 寿元 " + kind.lifespanCostYears() + " 年";
        long readyAt = snapshot.readyAtTick(kind);
        if (readyAt <= nowTick) {
            return kind.label() + " | " + cost + " | 可用";
        }
        return kind.label() + " | " + cost + " | 冷却至 t" + readyAt;
    }

    static long nowTick() {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.world != null) {
            return mc.world.getTime();
        }
        return System.currentTimeMillis() / 50L;
    }
}
