package com.bong.client.inventory.component;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ItemTooltipPanelTest {
    @Test
    void spiritQualityLabelAndBarClampToTooltipWidth() {
        InventoryItem item = InventoryItem.createFull(
            7L,
            "kaimai_dan",
            "开脉丹",
            1,
            1,
            0.2,
            "rare",
            "",
            1,
            0.72,
            1.0
        );

        assertEquals("灵质 72%", ItemTooltipPanel.spiritQualityLabel(item));
        assertEquals(141, ItemTooltipPanel.qualityBarFillWidth(196, item.spiritQuality()));
        assertEquals(0, ItemTooltipPanel.qualityBarFillWidth(196, -1.0));
        assertEquals(196, ItemTooltipPanel.qualityBarFillWidth(196, 2.0));
    }

    @Test
    void spiritQualityLabelRespondsToUpdatedSnapshotItem() {
        InventoryItem before = InventoryItem.createFull(
            7L,
            "ling_stone",
            "碎灵石",
            1,
            1,
            0.2,
            "common",
            "",
            1,
            0.97,
            1.0
        );
        InventoryItem after = InventoryItem.createFull(
            7L,
            "ling_stone",
            "碎灵石",
            1,
            1,
            0.2,
            "common",
            "",
            1,
            0.68,
            1.0
        );

        assertEquals("灵质 97%", ItemTooltipPanel.spiritQualityLabel(before));
        assertEquals("灵质 68%", ItemTooltipPanel.spiritQualityLabel(after));
        int widthBefore = ItemTooltipPanel.qualityBarFillWidth(196, before.spiritQuality());
        int widthAfter = ItemTooltipPanel.qualityBarFillWidth(196, after.spiritQuality());
        assertTrue(
            widthAfter < widthBefore,
            () -> "expected fillWidth(after) < fillWidth(before) because spiritQuality dropped, actual after="
                + widthAfter
                + " before="
                + widthBefore
        );
    }

    @Test
    void qualityBarColorMovesFromGreyThroughGreenToGold() {
        assertEquals(0x888888, ItemTooltipPanel.qualityBarColor(0.0));
        assertEquals(0x22CC22, ItemTooltipPanel.qualityBarColor(0.5));
        assertEquals(0xFFAA00, ItemTooltipPanel.qualityBarColor(1.0));
    }

    @Test
    void ancientRelicStatusIncludesChargesWarning() {
        InventoryItem relic = InventoryItem.createFullWithVisualMeta(
            77L,
            "ancient_broken_blade",
            "上古断刃",
            1,
            2,
            1.0,
            "ancient",
            "",
            1,
            0.0,
            1.0,
            3,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );

        String status = ItemTooltipPanel.formatStatusLine(relic);
        assertTrue(status.contains("⚡ ×3"));
        assertTrue(status.contains("上古遗物·一次性"));
    }

    @Test
    void mundaneArmorTooltipLinesShowMaterialDefenseAndRepairHint() {
        InventoryItem armor = InventoryItem.createFull(
            88L,
            "armor_copper_chestplate",
            "铜甲胸甲",
            2,
            2,
            2.1,
            "common",
            "",
            1,
            1.0,
            0.0
        );

        assertEquals("凡物·铜制", ItemTooltipPanel.armorMaterialLine(armor));
        assertEquals("防御: +2.80", ItemTooltipPanel.armorDefenseLine(armor));
        assertEquals("已损坏·不可穿戴", ItemTooltipPanel.armorBrokenLine(armor));
        assertEquals("修复: 同材质 ×2 hand-craft", ItemTooltipPanel.armorRepairLine(armor));
    }

    @Test
    void rarityLabelsCoverAllSixTiers() {
        assertRarityLabelAndColor("common", "普通", 0x808080);
        assertRarityLabelAndColor("uncommon", "精良", 0x22CC22);
        assertRarityLabelAndColor("rare", "稀有", 0x2288FF);
        assertRarityLabelAndColor("epic", "史诗", 0xAA44FF);
        assertRarityLabelAndColor("legendary", "传说", 0xFFAA00);
        assertRarityLabelAndColor(" Ancient ", "上古", 0xFF4444);
    }

    private static void assertRarityLabelAndColor(String rarity, String label, int color) {
        InventoryItem item = InventoryItem.createFull(
            99L,
            "fixture",
            "fixture",
            1,
            1,
            0.1,
            rarity,
            "",
            1,
            1.0,
            1.0
        );

        assertEquals(label, ItemTooltipPanel.rarityLabel(rarity));
        assertEquals(color, item.rarityColor());
    }

    // ─── plan-inventory-hint-panel-v1 P2：装备槽 hover"约束说明"区 ────────────────
    // slotConstraintLine/slotConstraintColor 是纯函数契约（不依赖 MinecraftClient，可直测）；
    // setHoveredEquipSlot 的实例状态只安全覆盖"空槽"路径（非空 item 的 computeRequiredHeight
    // 会触达 MinecraftClient.getInstance()，单测环境无 MC 启动会 NPE —— 与既有
    // formatStatusLine 等纯函数测试同一约束，见类头注释）。

    // --- 身体槽（worn cap）：未满 → 绿 / 恰好满 → 红 + "已满" 字面量，HEAD/FEET/CHEST/LEGS 各一条边界。

    @Test
    void headSlotBelowCapShowsProgressInOkColor() {
        assertEquals("可叠 0/2 层", ItemTooltipPanel.slotConstraintLine(EquipSlotType.HEAD, 0));
        assertEquals(ItemTooltipPanel.CONSTRAINT_OK_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.HEAD, 0));
        assertEquals("可叠 1/2 层", ItemTooltipPanel.slotConstraintLine(EquipSlotType.HEAD, 1));
        assertEquals(ItemTooltipPanel.CONSTRAINT_OK_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.HEAD, 1));
    }

    @Test
    void headSlotAtCapShowsFullInFullColor() {
        String line = ItemTooltipPanel.slotConstraintLine(EquipSlotType.HEAD, 2);
        assertTrue(line.contains("已满"), () -> "worn_cap 满的槽 hover 须显示 '已满'（plan §P2 测试声明），实际=" + line);
        assertTrue(line.contains("已穿戴 2 层，无法再叠加"),
            () -> "满员措辞须与 P1 toast capFullMessage 语感一致（'已穿戴 N 层，无法再叠加'），实际=" + line);
        assertEquals(ItemTooltipPanel.CONSTRAINT_FULL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.HEAD, 2));
    }

    @Test
    void feetSlotCapBoundaryMatchesHead() {
        assertEquals("可叠 1/2 层", ItemTooltipPanel.slotConstraintLine(EquipSlotType.FEET, 1));
        assertEquals(ItemTooltipPanel.CONSTRAINT_OK_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.FEET, 1));
        assertTrue(ItemTooltipPanel.slotConstraintLine(EquipSlotType.FEET, 2).contains("已满"));
        assertEquals(ItemTooltipPanel.CONSTRAINT_FULL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.FEET, 2));
    }

    @Test
    void chestSlotBelowCapShowsProgressInOkColor() {
        assertEquals("可叠 0/3 层", ItemTooltipPanel.slotConstraintLine(EquipSlotType.CHEST, 0));
        assertEquals("可叠 2/3 层", ItemTooltipPanel.slotConstraintLine(EquipSlotType.CHEST, 2));
        assertEquals(ItemTooltipPanel.CONSTRAINT_OK_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.CHEST, 2));
    }

    @Test
    void chestSlotAtCapShowsFullInFullColor() {
        String line = ItemTooltipPanel.slotConstraintLine(EquipSlotType.CHEST, 3);
        assertTrue(line.contains("已满"));
        assertTrue(line.contains("已穿戴 3 层，无法再叠加"));
        assertEquals(ItemTooltipPanel.CONSTRAINT_FULL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.CHEST, 3));
    }

    @Test
    void legsSlotCapBoundaryMatchesChest() {
        assertEquals("可叠 2/3 层", ItemTooltipPanel.slotConstraintLine(EquipSlotType.LEGS, 2));
        assertEquals(ItemTooltipPanel.CONSTRAINT_OK_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.LEGS, 2));
        assertTrue(ItemTooltipPanel.slotConstraintLine(EquipSlotType.LEGS, 3).contains("已满"));
        assertEquals(ItemTooltipPanel.CONSTRAINT_FULL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.LEGS, 3));
    }

    @Test
    void overflowWornCountBeyondCapStillReportsFullUsingCapNotCount() {
        // 防御性 off-by-one：wornCount(3) > cap(2) 理论不该发生（server 是唯一真源禁止超装），
        // 但 hover 面板须仍稳定报"已满"且措辞用 cap 值（2），不能把越界的 count 值（3）拼进文案。
        String line = ItemTooltipPanel.slotConstraintLine(EquipSlotType.HEAD, 3);
        assertTrue(line.contains("已满"));
        assertTrue(line.contains("已穿戴 2 层，无法再叠加"),
            () -> "满员文案须用 cap(2) 而非越界 count(3) 拼句，实际=" + line);
        assertEquals(ItemTooltipPanel.CONSTRAINT_FULL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.HEAD, 3));
    }

    // --- 手槽（held-only，无 worn cap 概念）：4 个手槽变体，恒返回持械位提示 + 中性色，不受 wornCount 影响。

    @Test
    void mainHandAlwaysReportsHeldSlotRegardlessOfWornCount() {
        assertEquals("持械位 · 仅可持 1 件", ItemTooltipPanel.slotConstraintLine(EquipSlotType.MAIN_HAND, 0));
        assertEquals("持械位 · 仅可持 1 件", ItemTooltipPanel.slotConstraintLine(EquipSlotType.MAIN_HAND, 5));
        assertEquals(ItemTooltipPanel.CONSTRAINT_NEUTRAL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.MAIN_HAND, 0));
    }

    @Test
    void offHandAlwaysReportsHeldSlot() {
        assertEquals("持械位 · 仅可持 1 件", ItemTooltipPanel.slotConstraintLine(EquipSlotType.OFF_HAND, 0));
        assertEquals(ItemTooltipPanel.CONSTRAINT_NEUTRAL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.OFF_HAND, 1));
    }

    @Test
    void extraHand0AlwaysReportsHeldSlot() {
        assertEquals("持械位 · 仅可持 1 件", ItemTooltipPanel.slotConstraintLine(EquipSlotType.EXTRA_HAND_0, 0));
        assertEquals(ItemTooltipPanel.CONSTRAINT_NEUTRAL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.EXTRA_HAND_0, 0));
    }

    @Test
    void extraHand1AlwaysReportsHeldSlot() {
        assertEquals("持械位 · 仅可持 1 件", ItemTooltipPanel.slotConstraintLine(EquipSlotType.EXTRA_HAND_1, 0));
        assertEquals(ItemTooltipPanel.CONSTRAINT_NEUTRAL_COLOR, ItemTooltipPanel.slotConstraintColor(EquipSlotType.EXTRA_HAND_1, 0));
    }

    // ─── setHoveredEquipSlot / setHoveredItem 状态契约（仅安全覆盖空槽路径，见类头注释）───

    @Test
    void setHoveredEquipSlotOnEmptySlotRecordsSlotIdentityWithNullItem() {
        ItemTooltipPanel panel = new ItemTooltipPanel();
        panel.setHoveredEquipSlot(EquipSlotType.CHEST, SlotContents.empty());

        assertNull(panel.hoveredItemForTest(), "空槽 representative 应为 null（不出物品面板）");
        assertEquals(EquipSlotType.CHEST, panel.hoveredSlotTypeForTest(), "槽身份须被记录（核心价值：空槽仍能预警）");
        assertEquals(0, panel.hoveredSlotWornCountForTest());
        assertEquals(112, panel.currentHeightForTest(), "空槽路径不触达 MinecraftClient，高度落回 DEFAULT_HEIGHT");
    }

    @Test
    void setHoveredEquipSlotWithNullContentsTreatedAsEmptyDefensively() {
        ItemTooltipPanel panel = new ItemTooltipPanel();
        panel.setHoveredEquipSlot(EquipSlotType.HEAD, null);

        assertNull(panel.hoveredItemForTest(), "contents==null 防 NPE，等价空槽");
        assertEquals(EquipSlotType.HEAD, panel.hoveredSlotTypeForTest());
        assertEquals(0, panel.hoveredSlotWornCountForTest());
    }

    @Test
    void setHoveredItemAfterEquipSlotHoverClearsSlotContext() {
        // 状态转换：hover 从装备槽移到"无 hover"（如移到空白区域）时，槽上下文须清空，
        // 否则约束面板会残留上一次的装备槽提示（"槽身份幽灵"）。
        ItemTooltipPanel panel = new ItemTooltipPanel();
        panel.setHoveredEquipSlot(EquipSlotType.LEGS, SlotContents.empty());
        assertEquals(EquipSlotType.LEGS, panel.hoveredSlotTypeForTest());

        panel.setHoveredItem(null);

        assertNull(panel.hoveredSlotTypeForTest(), "离开装备槽后槽上下文须重置为 null");
        assertNull(panel.hoveredItemForTest());
    }

    @Test
    void switchingBetweenTwoEmptyEquipSlotsUpdatesIdentityEachTime() {
        // A→B→A 状态转换（enum 变体切换）：连续 hover 不同空槽，身份逐次刷新，不粘滞前一个槽。
        ItemTooltipPanel panel = new ItemTooltipPanel();

        panel.setHoveredEquipSlot(EquipSlotType.HEAD, SlotContents.empty());
        assertEquals(EquipSlotType.HEAD, panel.hoveredSlotTypeForTest());

        panel.setHoveredEquipSlot(EquipSlotType.FEET, SlotContents.empty());
        assertEquals(EquipSlotType.FEET, panel.hoveredSlotTypeForTest());

        panel.setHoveredEquipSlot(EquipSlotType.HEAD, SlotContents.empty());
        assertEquals(EquipSlotType.HEAD, panel.hoveredSlotTypeForTest());
    }

    // ─── plan-inventory-hint-panel-v1 §P3：视听规格收尾（面板 chrome 4-tick 淡入）────────────
    // targetChanged/fadeProgress/applyFadeAlpha 均为纯函数契约（不依赖 System.currentTimeMillis
    // 或 MinecraftClient），供测试直接饱和断言边界，不绑定 draw() 内部像素调用序列。

    private static InventoryItem fixtureItem(long instanceId, String itemId) {
        return InventoryItem.createFull(instanceId, itemId, itemId, 1, 1, 0.1, "common", "", 1, 1.0, 1.0);
    }

    @Test
    void targetChangedFalseWhenSameItemAndSlotRepeated() {
        InventoryItem item = fixtureItem(1L, "kaimai_dan");
        // 同一 instanceId/内容两次独立构造的对象须 equals() 相等（value equality，非引用），
        // 否则每帧都会把淡入计时器错误复位，永远卡在 progress=0（面板全透明）。
        InventoryItem sameContentAgain = fixtureItem(1L, "kaimai_dan");
        assertEquals(item, sameContentAgain, "InventoryItem.equals 须走值语义，否则淡入计时器永不稳定");

        assertEquals(false, ItemTooltipPanel.targetChanged(item, EquipSlotType.CHEST, sameContentAgain, EquipSlotType.CHEST),
            "item 内容相同 + slotType 相同 → 未变化，不应复位淡入计时器");
    }

    @Test
    void targetChangedTrueWhenItemDiffers() {
        InventoryItem a = fixtureItem(1L, "kaimai_dan");
        InventoryItem b = fixtureItem(2L, "ling_stone");
        assertEquals(true, ItemTooltipPanel.targetChanged(a, null, b, null),
            "instanceId/itemId 不同的两件物品须判定为目标变化");
    }

    @Test
    void targetChangedTrueWhenSlotTypeDiffersWithSameItem() {
        InventoryItem item = fixtureItem(1L, "kaimai_dan");
        assertEquals(true, ItemTooltipPanel.targetChanged(item, EquipSlotType.HEAD, item, EquipSlotType.CHEST),
            "item 相同但 slotType 从 HEAD 切到 CHEST（如 hover 跨槽拖过） → 须判定为变化");
    }

    @Test
    void targetChangedTrueWhenTransitioningBetweenNullAndNonNullItem() {
        InventoryItem item = fixtureItem(1L, "kaimai_dan");
        assertEquals(true, ItemTooltipPanel.targetChanged(null, null, item, null), "null→非 null item 须判定为变化");
        assertEquals(true, ItemTooltipPanel.targetChanged(item, null, null, null), "非 null→null item 须判定为变化");
    }

    @Test
    void targetChangedFalseWhenBothItemAndSlotAreNull() {
        assertEquals(false, ItemTooltipPanel.targetChanged(null, null, null, null),
            "hint 面板持续无 hover（item/slot 均为 null）不应反复复位淡入计时器");
    }

    @Test
    void fadeProgressAtOrBeforeZeroElapsedIsFullyTransparent() {
        assertEquals(0f, ItemTooltipPanel.fadeProgress(0L), "elapsed=0（刚复位）→ progress=0");
        assertEquals(0f, ItemTooltipPanel.fadeProgress(-5L), "负 elapsed（防御性，理论不该发生）仍 clamp 到 0，不越界为负");
    }

    @Test
    void fadeProgressAtDurationBoundaryIsFullyOpaque() {
        assertEquals(1f, ItemTooltipPanel.fadeProgress(ItemTooltipPanel.FADE_DURATION_MILLIS),
            "elapsed 恰好等于 FADE_DURATION_MILLIS（4 tick=200ms）→ progress=1（边界含）");
        assertEquals(1f, ItemTooltipPanel.fadeProgress(ItemTooltipPanel.FADE_DURATION_MILLIS + 1000L),
            "elapsed 远超过淡入时长 → 仍 clamp 到 1，不越界为 >1");
    }

    @Test
    void fadeProgressMidpointIsLinearInterpolation() {
        long half = ItemTooltipPanel.FADE_DURATION_MILLIS / 2;
        assertEquals(0.5f, ItemTooltipPanel.fadeProgress(half), 0.001f,
            "淡入时长过半 → progress 线性插值到 0.5");
    }

    @Test
    void applyFadeAlphaAtZeroProgressStripsAlphaButKeepsRgb() {
        int result = ItemTooltipPanel.applyFadeAlpha(0xE0141414, 0f);
        assertEquals(0x00141414, result, "progress=0 → alpha 通道清零（全透明），RGB 不变");
    }

    @Test
    void applyFadeAlphaAtFullProgressPreservesOriginalColor() {
        int result = ItemTooltipPanel.applyFadeAlpha(0xE0141414, 1f);
        assertEquals(0xE0141414, result, "progress=1（淡入完成）→ 原色不变，含原 alpha 0xE0");
    }

    @Test
    void applyFadeAlphaAtHalfProgressHalvesBaseAlpha() {
        // BORDER_COLOR 基础 alpha=0xFF(255)；半程淡入 → alpha≈127~128（Math.round(255*0.5)=128）。
        int result = ItemTooltipPanel.applyFadeAlpha(0xFF3A3A3A, 0.5f);
        int alpha = (result >>> 24) & 0xFF;
        assertEquals(128, alpha, "基础 alpha=255 时半程淡入 → Math.round(255*0.5)=128");
        assertEquals(0x3A3A3A, result & 0x00FFFFFF, "RGB 通道须原样保留，不受淡入影响");
    }

    @Test
    void applyFadeAlphaClampsOutOfRangeProgress() {
        // 防御性边界：progress 理论上不该越界（fadeProgress 已 clamp），但 applyFadeAlpha 作为
        // 独立可测契约，自身也须对越界输入防御，不产生越界 alpha（<0 或 >255）。
        assertEquals(0x00141414, ItemTooltipPanel.applyFadeAlpha(0xE0141414, -0.5f), "progress<0 → clamp 到 0");
        assertEquals(0xE0141414, ItemTooltipPanel.applyFadeAlpha(0xE0141414, 1.5f), "progress>1 → clamp 到 1");
    }
}
