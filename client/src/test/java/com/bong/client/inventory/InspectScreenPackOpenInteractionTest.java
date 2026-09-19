package com.bong.client.inventory;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import java.util.EnumMap;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 工具在装备槽和快捷栏之间的领域限制回归。 */
public class InspectScreenPackOpenInteractionTest {

    private static InventoryItem item(long instanceId, String itemId) {
        return InventoryItem.createFull(
            instanceId, itemId, itemId, 1, 1, 0.1, "common", "", 1, 1.0, 1.0);
    }


    // ── plan-zhenfa-trap-client-equip-gate-v1 P1 —— warning_trap 装备槽落位 + hotbar 落点回归 ──
    //
    // InspectScreen.isEquipSlotDropValid()（3592 行）拖拽落位判定 = isEquipSlotUsable(target)
    // && InventoryEquipRules.canEquip(...)（无 bodyInspect 数据时 isEquipSlotUsable 恒 true，与本测试
    // 空装备态场景一致）；updateHighlights() 的 hotbar 落点分支（约 3400-3405 行）直接转调
    // InventoryEquipRules.canPlaceIntoHotbar(dragged)。两个私有方法都只是薄封装，owo 真实拖拽命中测试
    // 需要真实布局（headless 无法构造，与本文件顶部双击测试同口径，由 gradle build + 真机验证兜底）。
    // 这里锁住它们唯一依赖的可达谓词：P0 补录 TOOL_TEMPLATE_IDS 后应放行装备槽、拒绝 hotbar；
    // 若未来重构改动 InspectScreen 调用链而悄悄脱钩（比如换了别的谓词），这条测试仍然只测契约，不测调用链本身，
    // 因此不会误报——但一旦 warning_trap 真的无法拖入装备槽，这里必须先撞红。

    @Test
    void warningTrapDropIntoMainHandEquipSlotSucceeds() {
        InventoryItem trap = item(300L, "warning_trap");

        assertTrue(
            InventoryEquipRules.canEquip(trap, EquipSlotType.MAIN_HAND, null, new EnumMap<>(EquipSlotType.class)),
            "期望 warning_trap 拖入 MAIN_HAND 装备槽成功（isEquipSlotDropValid 直接转调此谓词），"
                + "实际被拒——说明 P0 TOOL_TEMPLATE_IDS 补录未生效或 InspectScreen 调用链脱钩");
    }

    @Test
    void warningTrapCannotDropIntoHotbar() {
        InventoryItem trap = item(301L, "warning_trap");

        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(trap),
            "期望 warning_trap 落入 hotbar 被拒（updateHighlights hotbar 分支直接转调 canPlaceIntoHotbar，"
                + "与 server ItemCategory::Tool forbidden_in_hotbar 契约对齐），实际放行——会给玩家误导性绿灯高亮");
    }
}
