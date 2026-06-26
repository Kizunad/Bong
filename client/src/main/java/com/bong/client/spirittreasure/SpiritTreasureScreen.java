package com.bong.client.spirittreasure;

import com.bong.client.hud.BongToast;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;

public final class SpiritTreasureScreen extends Screen {
    private static final int PANEL_WIDTH = 340;
    private static final int PANEL_HEIGHT = 242;
    private static final int TAB_WIDTH = 88;
    private static final String JIZHAOJING_TEMPLATE_ID = "spirit_treasure_jizhaojing";

    // plan-layered-equip-v1 P4（决议 #8）— 触发位区：容量默认 4，与 server TREASURE_TRIGGER_CAP 对齐。
    private static final int TRIGGER_CAP = 4;
    private static final int TRIGGER_SLOT_W = 56;
    private static final int TRIGGER_SLOT_H = 34;
    private static final int TRIGGER_SLOT_GAP = 8;
    private static final int FLASH_COLOR = 0xFF7FD2FF; // 落位淡青闪
    private static final long FLY_IN_MS = 600L; // ~12 tick @ 20tps
    private static final long FLASH_MS = 300L; // 落位边框 6tick fade
    private static final long TOAST_DURATION_MS = 3000L; // 淡入 10t / 停留 40t / 淡出 10t ≈ 3s
    private static final int TOAST_COLOR = 0xBFE6FF;

    private String selectedTemplateId = "";

    // 触发位飞入动画（一次一件）。
    private long flyInStartMs = 0L;
    private long flyInEndMs = 0L;
    private int flyInFromX = 0;
    private int flyInFromY = 0;
    private int flyInToX = 0;
    private int flyInToY = 0;
    private int flyInTargetSlot = -1;
    private long flashStartMs = 0L;
    private int flashSlot = -1;
    private String flyInLabel = "";

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
            panel.render(context, textRenderer, left + 8, top + 62, PANEL_WIDTH - 16, PANEL_HEIGHT - 138);
        }

        renderTriggerSlots(context, left, top, treasures);
        super.render(context, mouseX, mouseY, delta);
    }

    // ─── 触发位区（决议 #8）─────────────────────────────────────────────

    private int triggerRowTop(int top) {
        return top + PANEL_HEIGHT - TRIGGER_SLOT_H - 14;
    }

    private int triggerSlotX(int left, int index) {
        int rowWidth = TRIGGER_CAP * TRIGGER_SLOT_W + (TRIGGER_CAP - 1) * TRIGGER_SLOT_GAP;
        int startX = left + (PANEL_WIDTH - rowWidth) / 2;
        return startX + index * (TRIGGER_SLOT_W + TRIGGER_SLOT_GAP);
    }

    private void renderTriggerSlots(DrawContext context, int left, int top, List<SpiritTreasureState> treasures) {
        int slotY = triggerRowTop(top);
        context.drawTextWithShadow(
            textRenderer, Text.literal("触发位（右键灵宝激活/卸下）"),
            left + 12, slotY - 14, 0xFF9FC8E0
        );

        List<SpiritTreasureState> active = activeTreasures(treasures);
        long now = System.currentTimeMillis();
        for (int i = 0; i < TRIGGER_CAP; i++) {
            int x = triggerSlotX(left, i);
            context.fill(x, slotY, x + TRIGGER_SLOT_W, slotY + TRIGGER_SLOT_H, 0xCC10161C);
            int border = 0xFF3A5060;
            // 落位闪：本槽刚被激活 → 淡青边框 fade。
            if (flashSlot == i && now - flashStartMs < FLASH_MS) {
                border = FLASH_COLOR;
            }
            context.drawBorder(x, slotY, TRIGGER_SLOT_W, TRIGGER_SLOT_H, border);

            if (i < active.size()) {
                SpiritTreasureState t = active.get(i);
                // 飞入动画期间，目标槽内容暂不画（由飞入字形代替），落位后再常驻。
                boolean flying = flyInTargetSlot == i && now < flyInEndMs;
                if (!flying) {
                    String label = clip(t.displayName(), 4);
                    int tx = x + (TRIGGER_SLOT_W - textRenderer.getWidth(label)) / 2;
                    context.drawTextWithShadow(textRenderer, Text.literal(label), tx, slotY + 12, 0xFFE6F2FF);
                }
            }
        }

        renderFlyIn(context, now);
    }

    private void renderFlyIn(DrawContext context, long now) {
        if (flyInTargetSlot < 0 || now >= flyInEndMs) {
            return;
        }
        double t = (now - flyInStartMs) / (double) Math.max(1L, flyInEndMs - flyInStartMs);
        t = Math.max(0.0, Math.min(1.0, t));
        double eased = 1.0 - Math.pow(1.0 - t, 3.0); // ease-out cubic
        int x = (int) Math.round(flyInFromX + (flyInToX - flyInFromX) * eased);
        int y = (int) Math.round(flyInFromY + (flyInToY - flyInFromY) * eased);
        String label = clip(flyInLabel, 4);
        context.drawTextWithShadow(textRenderer, Text.literal(label), x, y, 0xFF7FD2FF);
    }

    private static List<SpiritTreasureState> activeTreasures(List<SpiritTreasureState> all) {
        List<SpiritTreasureState> active = new ArrayList<>();
        for (SpiritTreasureState t : all) {
            if (t.passiveActive()) {
                active.add(t);
            }
        }
        return active;
    }

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        // 右键 = 激活/卸下灵宝到触发位（决议 #8）。
        if (button == 1) {
            List<SpiritTreasureState> treasures = SpiritTreasureStateStore.snapshot();
            int left = (width - PANEL_WIDTH) / 2;
            int top = Math.max(18, (height - PANEL_HEIGHT) / 2);

            // 命中触发位槽 → 卸下该槽内激活法宝。
            int slotY = triggerRowTop(top);
            if (mouseY >= slotY && mouseY <= slotY + TRIGGER_SLOT_H) {
                List<SpiritTreasureState> active = activeTreasures(treasures);
                for (int i = 0; i < TRIGGER_CAP && i < active.size(); i++) {
                    int x = triggerSlotX(left, i);
                    if (mouseX >= x && mouseX <= x + TRIGGER_SLOT_W) {
                        SpiritTreasureState t = active.get(i);
                        ClientRequestSender.sendTreasureActivate(t.instanceId(), false);
                        BongToast.show(t.displayName() + " 已卸下", TOAST_COLOR,
                            System.currentTimeMillis(), TOAST_DURATION_MS);
                        return true;
                    }
                }
            }

            // 命中当前选中的（非激活）灵宝面板区 → 激活该灵宝。
            SpiritTreasureState selected = selected(treasures);
            if (selected != null && !selected.passiveActive() && mouseY < slotY - 16) {
                int targetSlot = activeTreasures(treasures).size();
                if (targetSlot >= TRIGGER_CAP) {
                    BongToast.show("触发位已满", 0xFFAA66, System.currentTimeMillis(), TOAST_DURATION_MS);
                    return true;
                }
                ClientRequestSender.sendTreasureActivate(selected.instanceId(), true);
                BongToast.show(selected.displayName() + " 已激活", TOAST_COLOR,
                    System.currentTimeMillis(), TOAST_DURATION_MS);
                startFlyIn((int) mouseX, (int) mouseY, left, top, targetSlot, selected.displayName());
                return true;
            }
        }
        return super.mouseClicked(mouseX, mouseY, button);
    }

    private void startFlyIn(int fromX, int fromY, int left, int top, int targetSlot, String label) {
        long now = System.currentTimeMillis();
        int slotY = triggerRowTop(top);
        int x = triggerSlotX(left, targetSlot);
        this.flyInStartMs = now;
        this.flyInEndMs = now + FLY_IN_MS;
        this.flyInFromX = fromX;
        this.flyInFromY = fromY;
        this.flyInToX = x + 10;
        this.flyInToY = slotY + 12;
        this.flyInTargetSlot = targetSlot;
        this.flyInLabel = label;
        this.flashStartMs = now + FLY_IN_MS; // 落位时才开始闪
        this.flashSlot = targetSlot;
    }

    private static String clip(String s, int maxChars) {
        if (s == null) {
            return "";
        }
        return s.length() <= maxChars ? s : s.substring(0, maxChars);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    private SpiritTreasureState selected(List<SpiritTreasureState> treasures) {
        for (SpiritTreasureState treasure : treasures) {
            if (treasure.templateId().equals(selectedTemplateId)) {
                return treasure;
            }
        }
        return treasures.isEmpty() ? null : treasures.get(0);
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
