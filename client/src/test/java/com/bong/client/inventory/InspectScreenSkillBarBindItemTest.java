package com.bong.client.inventory;

import com.bong.client.block.BlockPlaceIntentResolver;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class InspectScreenSkillBarBindItemTest {
    private record Sent(Identifier channel, String body) {}

    private static final Identifier CLIENT_REQUEST = new Identifier("bong", "client_request");

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        SkillBarStore.resetForTests();
        QuickUseSlotStore.resetForTests();
    }

    private void captureBackend() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void onlyBlockItemsOpenSkillBarBindMenu() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        assertTrue(screen.openSkillBarContextMenu(blockItem(), 10, 20));
        assertTrue(screen.hasOpenSkillBarContextMenu());
        assertFalse(screen.openSkillBarContextMenu(
            InventoryItem.createFull(2L, "guyuan_pill", "固元丹", 1, 1, 0.2, "rare", "", 1, 1.0, 1.0),
            10,
            20
        ));
    }

    @Test
    void bindBlockItemSendsRequestAndUpdatesLocalSkillBar() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        assertTrue(screen.bindBlockItemToSkillBar(2, blockItem()));

        assertEquals(1, sent.size());
        assertEquals(CLIENT_REQUEST, sent.get(0).channel());
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":2,\"binding\":{\"kind\":\"item\",\"template_id\":\"earth_crumb\"}}",
            sent.get(0).body()
        );
        SkillBarEntry entry = SkillBarStore.snapshot().slot(2);
        assertEquals(SkillBarEntry.Kind.ITEM, entry.kind());
        assertEquals("earth_crumb", entry.id());
        assertTrue(entry.iconTexture().endsWith("textures/gui/items/earth_crumb.png"));
    }

    // ---- P0: 拖放方块到快捷使用栏（commitQuickUseDrop）同时写 quick_slot + SkillBar ----

    @Test
    void dropBlockIntoQuickUseAlsoBindsSkillBar() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        // 模拟拖放落到快捷栏第 4 槽（index=3）的收口。
        screen.commitQuickUseDrop(3, blockItem());

        // quick_slot_bind 与 skill_bar_bind 两条 C2S 都发出（顺序：先 quick_slot 再 skill_bar）。
        assertEquals(2, sent.size(),
            "拖方块进快捷栏应同时发 quick_slot_bind + skill_bar_bind，实际发了 " + sent.size() + " 条");
        assertEquals(
            "{\"type\":\"quick_slot_bind\",\"v\":1,\"slot\":3,\"item_id\":\"earth_crumb\"}",
            sent.get(0).body());
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":3,\"binding\":{\"kind\":\"item\",\"template_id\":\"earth_crumb\"}}",
            sent.get(1).body());

        // 本地 SkillBarStore 槽被写成 ITEM（右键「绑定到 N」同效）。
        SkillBarEntry entry = SkillBarStore.snapshot().slot(3);
        assertNotNull(entry, "SkillBarStore 槽 3 应被写入 entry");
        assertEquals(SkillBarEntry.Kind.ITEM, entry.kind(),
            "拖方块进快捷栏后 SkillBarStore.slot(3) 应为 Kind.ITEM");
        assertEquals("earth_crumb", entry.id());
    }

    @Test
    void dropBlockIntoQuickUseEnablesBlockPlaceIntent() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        // 拖放收口：写 SkillBarStore.slot(2) = earth_crumb（block）。
        screen.commitQuickUseDrop(2, blockItem());
        // 玩家用 HUD 热键选中第 3 槽（1-9 选中栏 index=2）。
        SkillBarStore.setSelectedSlot(2);

        // 库存中存在该方块实例（hotbar 槽 2 携带 earth_crumb instance=1）。
        InventoryModel inventory = InventoryModel.builder()
            .hotbar(2, blockItem())
            .build();

        BlockPlaceIntentResolver.Intent intent = BlockPlaceIntentResolver.selectedBlockPlaceIntent(
            2, inventory, new BlockPos(10, 64, 20), Direction.UP);

        assertNotNull(intent,
            "拖方块进快捷栏并选中该槽后，selectedBlockPlaceIntent 应非空（拖放=可放置）");
        assertEquals(1L, intent.instanceId(),
            "intent 应指向被拖入方块的实例 id=1");
        assertEquals(new BlockPos(10, 65, 20), intent.placePos(),
            "UP 面放置点应在 targetPos 上方一格");
    }

    @Test
    void dropNonBlockIntoQuickUseOnlyBindsQuickSlot() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        InventoryItem pill =
            InventoryItem.createFull(9L, "guyuan_pill", "固元丹", 1, 1, 0.2, "rare", "", 1, 1.0, 1.0);
        screen.commitQuickUseDrop(5, pill);

        // 非方块物品：只发 quick_slot_bind，SkillBar 不动（行为回归）。
        assertEquals(1, sent.size(),
            "非方块物品拖进快捷栏只应发 quick_slot_bind，实际发了 " + sent.size() + " 条");
        assertEquals(
            "{\"type\":\"quick_slot_bind\",\"v\":1,\"slot\":5,\"item_id\":\"guyuan_pill\"}",
            sent.get(0).body());

        SkillBarEntry entry = SkillBarStore.snapshot().slot(5);
        assertNull(entry,
            "非方块物品不应写入 SkillBarStore.slot(5)，应保持为 null");
    }

    @Test
    void clearQuickUseSlotSendsNullQuickSlotAndLeavesSkillBarUntouched() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        // 拖空（item==null）= 清快捷栏槽：只发 quick_slot_bind(null)，不触碰 SkillBar。
        screen.commitQuickUseDrop(1, null);

        assertEquals(1, sent.size(),
            "清快捷栏槽只应发一条 quick_slot_bind(null)");
        assertEquals(
            "{\"type\":\"quick_slot_bind\",\"v\":1,\"slot\":1,\"item_id\":null}",
            sent.get(0).body());
        assertNull(SkillBarStore.snapshot().slot(1));
    }

    private static InventoryItem blockItem() {
        return InventoryItem.createFull(
            1L,
            "earth_crumb",
            "土块",
            1,
            1,
            0.1,
            "common",
            "可放置的土块",
            16,
            1.0,
            1.0
        );
    }
}
