package com.bong.client.inventory.component;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.SlotContents;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Positioning;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;

import java.util.EnumMap;
import java.util.Map;

/**
 * plan-layered-equip-v1 P4（决议 #12/#17）：装备面板，绝对定位摆 8 槽（删旧 11 槽里的 TWO_HAND/FALSE_SKIN/
 * TREASURE_BELT_0..3，新增 EXTRA_HAND_0/1，净减 2 槽）。
 *
 * <p><b>布局（中列一线 + 左右手两侧 + 多臂下方，PANEL_WIDTH=140, SLOT_SIZE=28, CX=70）：</b>
 * <pre>
 *   槽          (px, py)    说明
 *   ─────────────────────────────────────────────
 *   HEAD        (56,   4)   中列第 1（头甲）
 *   OFF_HAND    ( 8,  38)   左侧（左手 held），与 CHEST 行对齐
 *   CHEST       (56,  38)   中列第 2（胸甲 worn 栈：护甲/伪皮/背包件叠层）
 *   MAIN_HAND   (104, 38)   右侧（右手 held）
 *   EXTRA_HAND_0( 8,  72)   左下（多臂三），off_hand 正下方
 *   LEGS        (56,  72)   中列第 3（腿甲 worn 栈）
 *   EXTRA_HAND_1(104, 72)   右下（多臂四），main_hand 正下方
 *   FEET        (56, 106)   中列第 4（足甲 worn 栈）
 * </pre>
 * 中列纵向间距 34px（28 槽 + 6 间隙）；FEET 底边 = 106+28 = 134；worn 栈最多 +6px（3 层），
 * 层数角标 y-2，均落在 PANEL_HEIGHT=168 内。绝对定位避免 owo Sizing.fill(100) 顶飞兄弟节点
 *（memory feedback_owo_fill_overflow.md）。
 */
public class EquipmentPanel {
    private static final int PANEL_WIDTH = 140;
    private static final int PANEL_HEIGHT = 168;
    private static final int S = EquipSlotComponent.SLOT_SIZE; // 28
    private static final int CX = PANEL_WIDTH / 2;             // 70
    private static final int CENTER_X = CX - S / 2;            // 56（中列 x）
    private static final int SIDE_LEFT_X = 8;                  // 左手列 x
    private static final int SIDE_RIGHT_X = PANEL_WIDTH - S - 8; // 104（右手列 x）

    // 行 y 坐标。
    private static final int ROW_HEAD_Y = 4;
    private static final int ROW_HANDS_Y = 38;   // off/chest/main 同行
    private static final int ROW_EXTRA_Y = 72;   // extra0/legs/extra1 同行
    private static final int ROW_FEET_Y = 106;

    /**
     * 槽布局坐标表（单一真源）：EquipSlotType → {px, py}（绝对定位，相对面板左上角）。
     * 构造器据此 addSlot；测试据此核坐标/重叠/越界（owo 未 mount 时 x()/y() 不可读，故用此表核布局）。
     */
    static final EnumMap<EquipSlotType, int[]> SLOT_LAYOUT = new EnumMap<>(EquipSlotType.class);
    static {
        SLOT_LAYOUT.put(EquipSlotType.HEAD, new int[]{CENTER_X, ROW_HEAD_Y});
        SLOT_LAYOUT.put(EquipSlotType.CHEST, new int[]{CENTER_X, ROW_HANDS_Y});
        SLOT_LAYOUT.put(EquipSlotType.LEGS, new int[]{CENTER_X, ROW_EXTRA_Y});
        SLOT_LAYOUT.put(EquipSlotType.FEET, new int[]{CENTER_X, ROW_FEET_Y});
        SLOT_LAYOUT.put(EquipSlotType.OFF_HAND, new int[]{SIDE_LEFT_X, ROW_HANDS_Y});
        SLOT_LAYOUT.put(EquipSlotType.MAIN_HAND, new int[]{SIDE_RIGHT_X, ROW_HANDS_Y});
        SLOT_LAYOUT.put(EquipSlotType.EXTRA_HAND_0, new int[]{SIDE_LEFT_X, ROW_EXTRA_Y});
        SLOT_LAYOUT.put(EquipSlotType.EXTRA_HAND_1, new int[]{SIDE_RIGHT_X, ROW_EXTRA_Y});
    }

    static int panelWidth() { return PANEL_WIDTH; }
    static int panelHeight() { return PANEL_HEIGHT; }

    private final FlowLayout container;
    private final EnumMap<EquipSlotType, EquipSlotComponent> slotComponents = new EnumMap<>(EquipSlotType.class);

    public EquipmentPanel() {
        container = Containers.verticalFlow(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(PANEL_HEIGHT));
        container.surface(Surface.flat(0xFF181818));
        for (var entry : SLOT_LAYOUT.entrySet()) {
            addSlot(entry.getKey(), entry.getValue()[0], entry.getValue()[1]);
        }
    }

    private void addSlot(EquipSlotType type, int px, int py) {
        EquipSlotComponent slot = new EquipSlotComponent(type);
        slot.positioning(Positioning.absolute(px, py));
        slotComponents.put(type, slot);
        container.child(slot);
    }

    public FlowLayout container() { return container; }

    public EquipSlotComponent slotFor(EquipSlotType type) { return slotComponents.get(type); }

    public Map<EquipSlotType, EquipSlotComponent> allSlots() { return slotComponents; }

    /**
     * 从 model 填充各槽分层内容。multi-arm 槽（extra_hand）默认隐藏（无内容则不渲染图标，仅空槽标签）。
     * 双手武器在主手 held 时，对侧手槽置 disable。
     */
    public void populateFromModel(InventoryModel model) {
        Map<EquipSlotType, SlotContents> slots = model.equippedSlots();
        for (var entry : slotComponents.entrySet()) {
            SlotContents contents = slots.get(entry.getKey());
            entry.getValue().setContents(contents);
            entry.getValue().setDisabledByTwoHand(false);
        }
        applyTwoHandLock(slots);
    }

    /** 双手武器（spear/staff）在主手 held → 锁副手 + 多臂手槽（disable tint）。 */
    private void applyTwoHandLock(Map<EquipSlotType, SlotContents> slots) {
        SlotContents main = slots.get(EquipSlotType.MAIN_HAND);
        if (main == null || main.held() == null) return;
        if (!isTwoHandWeapon(main.held())) return;
        for (EquipSlotType hand : new EquipSlotType[]{
            EquipSlotType.OFF_HAND, EquipSlotType.EXTRA_HAND_0, EquipSlotType.EXTRA_HAND_1
        }) {
            EquipSlotComponent comp = slotComponents.get(hand);
            if (comp != null && comp.isEmpty()) {
                comp.setDisabledByTwoHand(true);
            }
        }
    }

    private static boolean isTwoHandWeapon(InventoryItem item) {
        if (item == null || item.itemId() == null) return false;
        String id = item.itemId().toLowerCase(java.util.Locale.ROOT);
        return id.contains("staff") || id.contains("spear");
    }

    public EquipSlotComponent slotAtScreen(double screenX, double screenY) {
        for (EquipSlotComponent slot : slotComponents.values()) {
            if (screenX >= slot.x() && screenX < slot.x() + EquipSlotComponent.SLOT_SIZE
                && screenY >= slot.y() && screenY < slot.y() + EquipSlotComponent.SLOT_SIZE) {
                return slot;
            }
        }
        return null;
    }

    public void clearHighlights() {
        for (EquipSlotComponent slot : slotComponents.values()) {
            slot.setHighlightState(GridSlotComponent.HighlightState.NONE);
        }
    }
}
