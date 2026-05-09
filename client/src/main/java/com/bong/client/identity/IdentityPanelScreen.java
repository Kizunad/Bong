package com.bong.client.identity;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.TextFieldWidget;
import net.minecraft.text.Text;

import java.util.List;

/** plan-identity-v1 P5：玩家身份面板，发 `/identity` slash command 到 server 权威校验。 */
public final class IdentityPanelScreen extends Screen {
    private static final int PANEL_WIDTH = 300;
    private static final int ROW_HEIGHT = 24;
    private static final int MAX_VISIBLE_IDENTITIES = 6;

    private TextFieldWidget nameField;

    public IdentityPanelScreen() {
        super(Text.literal("身份"));
    }

    @Override
    protected void init() {
        IdentityPanelState state = IdentityPanelStateStore.snapshot();
        int left = (width - PANEL_WIDTH) / 2;
        int top = Math.max(20, height / 2 - 108);

        nameField = new TextFieldWidget(textRenderer, left + 12, top + 40, PANEL_WIDTH - 24, 20, Text.literal("身份名"));
        nameField.setMaxLength(32);
        nameField.setPlaceholder(Text.literal("新身份 / 改名"));
        addDrawableChild(nameField);

        ButtonWidget createButton = ButtonWidget.builder(
            Text.literal("新建"),
            button -> sendIdentityCommand(newIdentityCommand(nameField.getText()))
        ).dimensions(left + 12, top + 64, 88, 20).build();
        createButton.active = state.cooldownPassed();
        addDrawableChild(createButton);

        addDrawableChild(ButtonWidget.builder(
            Text.literal("改名"),
            button -> sendIdentityCommand(renameIdentityCommand(nameField.getText()))
        ).dimensions(left + 106, top + 64, 88, 20).build());

        int rowY = top + 96;
        List<IdentityPanelEntry> entries = state.identities();
        int count = Math.min(entries.size(), MAX_VISIBLE_IDENTITIES);
        for (int i = 0; i < count; i++) {
            IdentityPanelEntry entry = entries.get(i);
            ButtonWidget switchButton = ButtonWidget.builder(
                Text.literal(entry.identityId() == state.activeIdentityId() ? "当前" : "切换"),
                button -> sendIdentityCommand(switchIdentityCommand(entry.identityId()))
            ).dimensions(left + PANEL_WIDTH - 84, rowY + i * ROW_HEIGHT - 2, 72, 20).build();
            switchButton.active = entry.identityId() != state.activeIdentityId() && state.cooldownPassed();
            addDrawableChild(switchButton);
        }
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        renderBackground(context);
        IdentityPanelState state = IdentityPanelStateStore.snapshot();
        int left = (width - PANEL_WIDTH) / 2;
        int top = Math.max(20, height / 2 - 108);

        context.fill(left, top, left + PANEL_WIDTH, top + 224, 0xCC101214);
        context.drawBorder(left, top, PANEL_WIDTH, 224, 0xFF4A4A55);
        context.drawTextWithShadow(textRenderer, Text.literal("身份"), left + 12, top + 12, 0xFFFFFFFF);
        context.drawTextWithShadow(
            textRenderer,
            Text.literal(cooldownLine(state)),
            left + 12,
            top + 26,
            state.cooldownPassed() ? 0xFF9FD3A0 : 0xFFFFC66D
        );

        renderIdentityRows(context, state, left, top + 96);
        super.render(context, mouseX, mouseY, delta);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    private void renderIdentityRows(DrawContext context, IdentityPanelState state, int left, int rowTop) {
        List<IdentityPanelEntry> entries = state.identities();
        if (entries.isEmpty()) {
            context.drawTextWithShadow(textRenderer, Text.literal("暂无身份数据"), left + 12, rowTop, 0xFFAAAAAA);
            return;
        }

        int count = Math.min(entries.size(), MAX_VISIBLE_IDENTITIES);
        for (int i = 0; i < count; i++) {
            IdentityPanelEntry entry = entries.get(i);
            int y = rowTop + i * ROW_HEIGHT;
            int color = entry.identityId() == state.activeIdentityId() ? 0xFFFFFFFF : 0xFFB8B8B8;
            context.drawTextWithShadow(textRenderer, Text.literal(formatEntryLine(entry, state.activeIdentityId())), left + 12, y, color);
        }
        if (entries.size() > MAX_VISIBLE_IDENTITIES) {
            context.drawTextWithShadow(
                textRenderer,
                Text.literal("另有 " + (entries.size() - MAX_VISIBLE_IDENTITIES) + " 个身份"),
                left + 12,
                rowTop + count * ROW_HEIGHT,
                0xFF888888
            );
        }
    }

    static String switchIdentityCommand(int identityId) {
        return "identity switch " + Math.max(0, identityId);
    }

    static String newIdentityCommand(String rawName) {
        return commandWithName("identity new", rawName);
    }

    static String renameIdentityCommand(String rawName) {
        return commandWithName("identity rename", rawName);
    }

    static String formatEntryLine(IdentityPanelEntry entry, int activeIdentityId) {
        String marker = entry.identityId() == activeIdentityId ? "*" : " ";
        String frozen = entry.frozen() ? " [冷藏]" : "";
        return marker + " #" + entry.identityId() + " " + entry.displayName() + frozen;
    }

    private static String commandWithName(String prefix, String rawName) {
        String name = sanitizeName(rawName);
        return name.isEmpty() ? "" : prefix + " " + name;
    }

    private static String sanitizeName(String rawName) {
        return rawName == null ? "" : rawName.trim().replaceAll("\\s+", " ");
    }

    private static String cooldownLine(IdentityPanelState state) {
        if (state.cooldownPassed()) {
            return "切换冷却：可用";
        }
        return "切换冷却：" + state.cooldownRemainingTicks() + " ticks";
    }

    private void sendIdentityCommand(String command) {
        if (command == null || command.isBlank()) {
            return;
        }
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.player != null && client.player.networkHandler != null) {
            client.player.networkHandler.sendCommand(command);
        }
        client.setScreen(null);
    }
}
