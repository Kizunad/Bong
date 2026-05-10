package com.bong.client.npc;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;

public final class NpcInspectScreen extends Screen {
    private final NpcMetadata metadata;

    public NpcInspectScreen(NpcMetadata metadata) {
        super(Text.literal(metadata == null ? "NPC" : metadata.displayName()));
        this.metadata = metadata;
    }

    @Override
    protected void init() {
        addDrawableChild(ButtonWidget.builder(Text.literal("返回"), button ->
                client.setScreen(new NpcDialogueScreen(metadata)))
            .dimensions(width / 2 - 64, height - 48, 58, 20)
            .build());
        addDrawableChild(ButtonWidget.builder(Text.literal("关闭"), button -> close())
            .dimensions(width / 2 + 6, height - 48, 58, 20)
            .build());
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        renderBackground(context);
        if (metadata != null) {
            int x = width / 2 - 116;
            int y = height / 2 - 78;
            context.drawTextWithShadow(textRenderer, "§e" + metadata.displayName(), x, y, 0xFFFFFF);
            y += 22;
            for (String line : lines(metadata)) {
                context.drawTextWithShadow(textRenderer, line, x, y, 0xD8D8D8);
                y += 14;
            }
        }
        super.render(context, mouseX, mouseY, delta);
    }

    private static List<String> lines(NpcMetadata metadata) {
        List<String> lines = new ArrayList<>();
        lines.add("§7类型 §f" + metadata.archetype());
        lines.add("§7境界 §f" + metadata.realm());
        if (metadata.factionName() != null) {
            lines.add("§7派系 §f" + metadata.factionName() + " / " + metadata.factionRank());
        }
        lines.add("§7寿元 §f" + metadata.ageBand());
        lines.add("§7态度 §f" + reputationLabel(metadata.reputationToPlayer()));
        if (metadata.qiHint() != null) {
            lines.add("§7望气 §f" + metadata.qiHint());
        }
        if (metadata.hostile()) {
            lines.add("§c此人对你充满敌意");
        }
        return lines;
    }

    private static String reputationLabel(int reputation) {
        if (reputation < -30) {
            return "敌意";
        }
        if (reputation > 50) {
            return "友善";
        }
        return "中立";
    }
}
