package com.bong.client.inventory.component;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-layered-equip-v1 P4（决议 #12）：EquipSlotComponent worn 栈 + held 数据 API 饱和测试。
 * 渲染（draw）需 MinecraftClient，单测只锁定可观察数据契约（栈顶/层数/代表件/disable）。
 */
class EquipSlotComponentTest {

    @Test
    void emptySlotReportsEmpty() {
        EquipSlotComponent c = new EquipSlotComponent(EquipSlotType.CHEST);
        assertTrue(c.isEmpty(), "新建槽应为空");
        assertNull(c.wornTop(), "空槽 wornTop 应为 null");
        assertNull(c.held(), "空槽 held 应为 null");
        assertNull(c.representative(), "空槽代表件应为 null");
    }

    @Test
    void wornTopIsStackTail() {
        // 决议 #12：栈顶 = worn 末尾。
        EquipSlotComponent c = new EquipSlotComponent(EquipSlotType.CHEST);
        c.setContents(new SlotContents(List.of(
            item(1L, "armor_bone_chestplate"),
            item(2L, "tuike_false_skin_silk"),
            item(3L, "worn_grass_pouch")
        ), null));

        assertEquals(3, c.contents().wornCount(), "应有 3 worn 层");
        assertEquals(3L, c.wornTop().instanceId(), "栈顶应为末尾件（instance 3）");
        assertEquals(3L, c.representative().instanceId(), "身体槽代表件 = worn 栈顶");
        assertFalse(c.isEmpty());
    }

    @Test
    void heldTakesPrecedenceForRepresentative() {
        EquipSlotComponent c = new EquipSlotComponent(EquipSlotType.MAIN_HAND);
        c.setContents(SlotContents.ofHeld(item(9L, "iron_sword")));
        assertEquals(9L, c.held().instanceId());
        assertEquals(9L, c.representative().instanceId(), "手槽代表件 = held");
        assertNull(c.wornTop(), "手槽无 worn");
    }

    @Test
    void clearContentsResetsToEmpty() {
        EquipSlotComponent c = new EquipSlotComponent(EquipSlotType.FEET);
        c.setContents(SlotContents.ofWorn(item(1L, "armor_bone_boots")));
        assertFalse(c.isEmpty());
        c.clearContents();
        assertTrue(c.isEmpty(), "clearContents 后应为空");
    }

    @Test
    void setNullContentsTreatedAsEmpty() {
        EquipSlotComponent c = new EquipSlotComponent(EquipSlotType.HEAD);
        c.setContents(null);
        assertTrue(c.isEmpty(), "setContents(null) 应等价空槽（防 NPE）");
    }

    @Test
    void disabledByTwoHandFlagTracks() {
        EquipSlotComponent c = new EquipSlotComponent(EquipSlotType.OFF_HAND);
        assertFalse(c.isDisabledByTwoHand(), "默认非 disable");
        c.setDisabledByTwoHand(true);
        assertTrue(c.isDisabledByTwoHand(), "双手锁定后应 disable");
        c.setDisabledByTwoHand(false);
        assertFalse(c.isDisabledByTwoHand());
    }

    @Test
    void slotTypeIsRetained() {
        assertEquals(EquipSlotType.EXTRA_HAND_0,
            new EquipSlotComponent(EquipSlotType.EXTRA_HAND_0).slotType());
    }

    private static InventoryItem item(long id, String itemId) {
        return InventoryItem.createFull(id, itemId, itemId, 1, 1, 1.0, "common", "", 1, 1.0, 1.0);
    }
}
