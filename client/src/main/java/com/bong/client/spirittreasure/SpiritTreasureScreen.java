package com.bong.client.spirittreasure;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.List;

public final class SpiritTreasureScreen extends Screen {
    private static final int PANEL_WIDTH = 340;
    private static final int PANEL_HEIGHT = 242;
    private static final int TAB_WIDTH = 88;
    private static final String JIZHAOJING_TEMPLATE_ID = "spirit_treasure_jizhaojing";

    private String selectedTemplateId = "";

    public SpiritTreasureScreen() {
        super(Text.literal("灵宝"));
    }

    @Override
    protected void init() {
        List<SpiritTreasureState> treasures = SpiritTreasureStateStore.snapshot();
        if (selectedTemplateId.isBlank() && !treasures.isEmpty()) {
            selectedTemplateId = treasures.get(0).templateId();
        }

        int left = (width - PANEL_WIDTH) / 2;
        int top = Math.max(18, (height - PANEL_HEIGHT) / 2);
        int tabX = left + 12;
        int tabY = top + 34;
        for (int i = 0; i < Math.min(treasures.size(), 3); i++) {
            SpiritTreasureState treasure = treasures.get(i);
            ButtonWidget button = ButtonWidget.builder(
                Text.literal(treasure.displayName()),
                ignored -> selectedTemplateId = treasure.templateId()
            ).dimensions(tabX + i * (TAB_WIDTH + 6), tabY, TAB_WIDTH, 20).build();
            addDrawableChild(button);
        }
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        renderBackground(context);
        int left = (width - PANEL_WIDTH) / 2;
        int top = Math.max(18, (height - PANEL_HEIGHT) / 2);
        context.fill(left, top, left + PANEL_WIDTH, top + PANEL_HEIGHT, 0xDD0C1014);
        context.drawBorder(left, top, PANEL_WIDTH, PANEL_HEIGHT, 0xFF405060);
        context.drawTextWithShadow(textRenderer, Text.literal("灵宝"), left + 12, top + 12, 0xFFFFFFFF);

        List<SpiritTreasureState> treasures = SpiritTreasureStateStore.snapshot();
        if (treasures.isEmpty()) {
            context.drawTextWithShadow(textRenderer, Text.literal("暂无灵宝"), left + 12, top + 64, 0xFFAAAAAA);
            super.render(context, mouseX, mouseY, delta);
            return;
        }

        SpiritTreasureState selected = selectTreasure(treasures);
        if (selected != null) {
            SpiritTreasureTabPanel panel = createPanel(selected);
            panel.render(context, textRenderer, left + 8, top + 62, PANEL_WIDTH - 16, PANEL_HEIGHT - 70);
        }
        super.render(context, mouseX, mouseY, delta);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    private SpiritTreasureState selectTreasure(List<SpiritTreasureState> treasures) {
        for (SpiritTreasureState treasure : treasures) {
            if (treasure.templateId().equals(selectedTemplateId)) {
                return treasure;
            }
        }
        SpiritTreasureState first = treasures.get(0);
        selectedTemplateId = first.templateId();
        return first;
    }

    private SpiritTreasureTabPanel createPanel(SpiritTreasureState state) {
        List<SpiritTreasureDialogue> dialogues = SpiritTreasureDialogueStore.recentFor(state.templateId());
        if (JIZHAOJING_TEMPLATE_ID.equals(state.templateId())) {
            return new JiZhaoJingTabPanel(state, dialogues);
        }
        return new JiZhaoJingTabPanel(state, dialogues);
    }
}
