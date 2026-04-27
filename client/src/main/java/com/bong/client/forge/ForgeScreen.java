package com.bong.client.forge;

import com.bong.client.forge.state.BlueprintScrollStore;
import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.forge.state.ForgeStationStore;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;

/**
 * plan-forge-v1 §3.3 MVP 锻炉 UI。
 *
 * 当前为基础占位：显示砧/会话/图谱/最近结果 store 状态。
 * 后续 UI（三列布局 + 节奏轨道 + 铭文槽 + 真元注入条）在后续切片中补全。
 */
public final class ForgeScreen extends Screen {

    public ForgeScreen() {
        super(Text.literal("锻炉"));
    }

    @Override
    protected void init() {
        super.init();
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        super.render(context, mouseX, mouseY, delta);

        int y = 20;
        int left = 12;

        ForgeStationStore.Snapshot station = ForgeStationStore.snapshot();
        context.drawText(textRenderer, Text.literal("§l砧: §r" + station.ownerName()
            + " tier=" + station.tier() + " 完整度=" + String.format("%.0f%%", station.integrity() * 100)
            + (station.hasSession() ? " §a[在炉]" : " §7[空闲]")),
            left, y, 0xFFFFFF, true);
        y += 14;

        ForgeSessionStore.Snapshot session = ForgeSessionStore.snapshot();
        if (session.sessionId() > 0) {
            context.drawText(textRenderer, Text.literal("§l会话: §r" + session.blueprintName()
                + " 步骤=" + session.currentStep() + " tier=" + session.achievedTier()),
                left, y, 0xFFFFFF, true);
            y += 14;
        }

        BlueprintScrollStore.Entry current = BlueprintScrollStore.current();
        if (current != null) {
            context.drawText(textRenderer, Text.literal("§l图谱: §r" + current.displayName()
                + " (tier_cap=" + current.tierCap() + " " + current.stepCount() + "步)"),
                left, y, 0xFFFFAA, true);
        } else {
            context.drawText(textRenderer, Text.literal("§l图谱: §7未学任何图谱"),
                left, y, 0xAAAAAA, true);
        }
        y += 14;

        ForgeOutcomeStore.Snapshot outcome = ForgeOutcomeStore.lastOutcome();
        if (outcome.sessionId() > 0) {
            String colorInfo = outcome.colorName() != null ? " 色=" + outcome.colorName() : " 无色";
            context.drawText(textRenderer, Text.literal("§l上次结果: §r" + outcome.bucket()
                + " " + (outcome.weaponItem() != null ? outcome.weaponItem() : "废料")
                + " 品质=" + String.format("%.0f%%", outcome.quality() * 100) + colorInfo
                + " tier=" + outcome.achievedTier()),
                left, y, outcome.flawedPath() ? 0xFFAA00 : 0x00FFAA, true);
            y += 14;
        }

        context.drawText(textRenderer, Text.literal("§7按 U 关闭 | 图谱翻页: ←/→"),
            left, y + 8, 0x888888, true);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        if (keyCode == 85) { // U
            this.close();
            return true;
        }
        if (keyCode == 263) { // ←
            BlueprintScrollStore.turn(-1);
            return true;
        }
        if (keyCode == 262) { // →
            BlueprintScrollStore.turn(1);
            return true;
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }
}
