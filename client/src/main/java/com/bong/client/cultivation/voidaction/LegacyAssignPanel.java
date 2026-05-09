package com.bong.client.cultivation.voidaction;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.TextFieldWidget;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;

public final class LegacyAssignPanel extends Screen {
    private static final int BG_COLOR = 0xD0101018;
    private static final int PANEL_COLOR = 0xE0181C24;
    private static final int TITLE_COLOR = 0xFFE9D9A6;
    private static final int TEXT_COLOR = 0xFFE8E8E8;
    private static final int MUTED_COLOR = 0xFF9AA4B2;
    private static final int ERROR_COLOR = 0xFFFF7777;

    private TextFieldWidget inheritorField;
    private TextFieldWidget itemIdsField;
    private TextFieldWidget messageField;
    private String errorText = "";

    public LegacyAssignPanel() {
        super(Text.literal("道统传承"));
    }

    @Override
    protected void init() {
        super.init();
        VoidActionStore.Snapshot snapshot = VoidActionStore.snapshot();
        int panelW = Math.min(390, Math.max(320, width - 40));
        int panelX = (width - panelW) / 2;
        int y = height / 2 - 40;

        inheritorField = new TextFieldWidget(textRenderer, panelX + 28, y, panelW - 56, 20, Text.literal("继承人"));
        inheritorField.setMaxLength(128);
        inheritorField.setText(snapshot.legacyInheritorId());
        addDrawableChild(inheritorField);

        itemIdsField = new TextFieldWidget(textRenderer, panelX + 28, y + 38, panelW - 56, 20, Text.literal("遗物 instance ids"));
        itemIdsField.setMaxLength(160);
        itemIdsField.setText(joinIds(snapshot.legacyItemInstanceIds()));
        addDrawableChild(itemIdsField);

        messageField = new TextFieldWidget(textRenderer, panelX + 28, y + 76, panelW - 56, 20, Text.literal("死信"));
        messageField.setMaxLength(512);
        messageField.setText(snapshot.legacyMessage() == null ? "" : snapshot.legacyMessage());
        addDrawableChild(messageField);

        this.addDrawableChild(ButtonWidget.builder(Text.literal("指定传承"), b -> submit())
            .dimensions(width / 2 - 76, y + 116, 72, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(Text.literal("返回"), b -> MinecraftClient.getInstance().setScreen(new VoidActionScreen()))
            .dimensions(width / 2 + 4, y + 116, 72, 20)
            .build());
        setInitialFocus(inheritorField);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        int panelW = Math.min(390, Math.max(320, width - 40));
        int panelH = 220;
        int panelX = (width - panelW) / 2;
        int panelY = (height - panelH) / 2;
        context.fill(panelX, panelY, panelX + panelW, panelY + panelH, PANEL_COLOR);
        context.drawCenteredTextWithShadow(textRenderer, "◇ 道 统 传 承 ◇", width / 2, panelY + 14, TITLE_COLOR);
        context.drawTextWithShadow(textRenderer, "继承人", panelX + 28, panelY + 42, TEXT_COLOR);
        context.drawTextWithShadow(textRenderer, "遗物 instance_id（逗号分隔）", panelX + 28, panelY + 80, TEXT_COLOR);
        context.drawTextWithShadow(textRenderer, "死信", panelX + 28, panelY + 118, TEXT_COLOR);
        if (!errorText.isBlank()) {
            context.drawCenteredTextWithShadow(textRenderer, errorText, width / 2, panelY + 178, ERROR_COLOR);
        } else {
            context.drawCenteredTextWithShadow(textRenderer, "继承人有 24h 可拒绝窗口", width / 2, panelY + 178, MUTED_COLOR);
        }
        super.render(context, mouseX, mouseY, delta);
    }

    private void submit() {
        List<Long> itemIds;
        try {
            itemIds = parseIds(itemIdsField.getText());
        } catch (IllegalArgumentException ex) {
            errorText = ex.getMessage();
            return;
        }
        String inheritor = inheritorField.getText();
        if (inheritor == null || inheritor.isBlank()) {
            errorText = "继承人不能为空";
            return;
        }
        String message = messageField.getText();
        VoidActionStore.setLegacyDraft(inheritor, itemIds, message);
        if (VoidActionHandler.dispatchLegacyAssign(inheritor, itemIds, message, VoidActionScreen.nowTick())) {
            MinecraftClient.getInstance().setScreen(null);
        }
    }

    static List<Long> parseIds(String raw) {
        if (raw == null || raw.isBlank()) return List.of();
        String[] chunks = raw.split(",");
        ArrayList<Long> ids = new ArrayList<>();
        for (String chunk : chunks) {
            String normalized = chunk.trim();
            if (normalized.isEmpty()) continue;
            long id;
            try {
                id = Long.parseLong(normalized);
            } catch (NumberFormatException ex) {
                throw new IllegalArgumentException("遗物 id 必须是整数");
            }
            if (id < 0L) {
                throw new IllegalArgumentException("遗物 id 不能为负");
            }
            ids.add(id);
        }
        return List.copyOf(ids);
    }

    private static String joinIds(List<Long> ids) {
        if (ids == null || ids.isEmpty()) return "";
        StringBuilder builder = new StringBuilder();
        for (Long id : ids) {
            if (builder.length() > 0) builder.append(',');
            builder.append(id);
        }
        return builder.toString();
    }
}
