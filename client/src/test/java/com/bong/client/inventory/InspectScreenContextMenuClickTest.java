package com.bong.client.inventory;

import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
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

/**
 * plan-block-placement-ux-v1 P2 — InspectScreen context menu（pill / skillBar / weapon）的
 * action 行命中要对左键(0)与右键(1)都生效；命中行之外的点击只有右键关闭菜单，左键不关。
 *
 * <p>契约（{@link InspectScreen#handleContextMenuClick}）：
 * <ul>
 *   <li>左键(0)点 action 行 → 触发该 action 并消费（返回 true）。</li>
 *   <li>右键(1)点 action 行 → 同样触发并消费。</li>
 *   <li>右键(1)点行外 → 关闭菜单并消费（返回 true）。</li>
 *   <li>左键(0)点行外 → 不关菜单、不消费（返回 false），让点击落到正常 UI 处理。</li>
 * </ul>
 *
 * <p>菜单几何：opened at (x=10, y=20)，每行高 16，padding 4，宽 112。
 * 行 0 命中区间 mouseY ∈ [24,40)、mouseX ∈ [10,122)；远处 (50,400) 为行外。
 */
class InspectScreenContextMenuClickTest {
    private record Sent(Identifier channel, String body) {}

    private static final int MENU_X = 10;
    private static final int MENU_Y = 20;
    // 行 0 中心：x 在菜单宽内，y = top + padding + half-row。
    private static final double ROW0_X = MENU_X + 20;
    private static final double ROW0_Y = MENU_Y + 4 + 8;
    // 行外：远在菜单下方。
    private static final double OUTSIDE_X = MENU_X + 20;
    private static final double OUTSIDE_Y = MENU_Y + 400;

    private static final int LEFT = 0;
    private static final int RIGHT = 1;

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        SkillBarStore.resetForTests();
    }

    private void captureBackend() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    // ---- skillBar 菜单：左键点 action 行触发绑定 ----

    @Test
    void leftClickOnSkillBarMenuActionTriggersBind() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        assertTrue(screen.openSkillBarContextMenu(blockItem(), MENU_X, MENU_Y));

        boolean consumed = screen.handleContextMenuClick(ROW0_X, ROW0_Y, LEFT);

        assertTrue(consumed, "左键点 skillBar 菜单 action 行应被消费");
        assertFalse(screen.hasOpenSkillBarContextMenu(), "触发 action 后菜单应关闭");
        // 行 0 = 「绑定到 1」= slot 0；触发后本地 SkillBarStore.slot(0) 写成 earth_crumb。
        SkillBarEntry entry = SkillBarStore.snapshot().slot(0);
        assertNotNull(entry, "左键触发绑定后 SkillBarStore.slot(0) 应非空");
        assertEquals(SkillBarEntry.Kind.ITEM, entry.kind());
        assertEquals("earth_crumb", entry.id());
        assertEquals(1, sent.size(), "应发一条 skill_bar_bind");
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":0,\"binding\":{\"kind\":\"item\",\"template_id\":\"earth_crumb\"}}",
            sent.get(0).body());
    }

    @Test
    void rightClickOnSkillBarMenuActionStillTriggersBind() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        assertTrue(screen.openSkillBarContextMenu(blockItem(), MENU_X, MENU_Y));

        boolean consumed = screen.handleContextMenuClick(ROW0_X, ROW0_Y, RIGHT);

        assertTrue(consumed, "右键点 skillBar 菜单 action 行仍应触发（向后兼容）");
        assertFalse(screen.hasOpenSkillBarContextMenu());
        SkillBarEntry entry = SkillBarStore.snapshot().slot(0);
        assertNotNull(entry);
        assertEquals("earth_crumb", entry.id());
    }

    @Test
    void rightClickOutsideSkillBarMenuClosesIt() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        assertTrue(screen.openSkillBarContextMenu(blockItem(), MENU_X, MENU_Y));

        boolean consumed = screen.handleContextMenuClick(OUTSIDE_X, OUTSIDE_Y, RIGHT);

        assertTrue(consumed, "右键点菜单外应被消费（用于关闭）");
        assertFalse(screen.hasOpenSkillBarContextMenu(), "右键点菜单外应关闭菜单");
        assertNull(SkillBarStore.snapshot().slot(0), "关闭菜单不应触发任何绑定");
    }

    @Test
    void leftClickOutsideSkillBarMenuDoesNotCloseOrConsume() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        assertTrue(screen.openSkillBarContextMenu(blockItem(), MENU_X, MENU_Y));

        boolean consumed = screen.handleContextMenuClick(OUTSIDE_X, OUTSIDE_Y, LEFT);

        assertFalse(consumed,
            "左键点菜单外不应被消费（应落到正常 UI 处理），避免误关菜单");
        assertTrue(screen.hasOpenSkillBarContextMenu(),
            "左键点菜单外不应关闭菜单");
    }

    // ---- pill 菜单：左键点 action 行触发服用 ----

    @Test
    void leftClickOnPillMenuActionTriggersSelfUse() {
        captureBackend();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        assertTrue(screen.openPillContextMenu(pillItem(), MENU_X, MENU_Y),
            "固元丹应能打开 pill 菜单（SELF_USE action）");

        boolean consumed = screen.handleContextMenuClick(ROW0_X, ROW0_Y, LEFT);

        assertTrue(consumed, "左键点 pill 菜单 action 行应被消费");
        assertFalse(screen.hasOpenPillContextMenu(), "触发后菜单应关闭");
        // SELF_USE → dispatchApplyPillSelf → apply_pill_self C2S。
        assertEquals(1, sent.size(), "应发一条 apply_pill 类 C2S");
        assertTrue(sent.get(0).body().contains("apply_pill"),
            "应发出 apply_pill 请求，实际 body=" + sent.get(0).body());
    }

    @Test
    void rightClickOutsidePillMenuClosesIt() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        assertTrue(screen.openPillContextMenu(pillItem(), MENU_X, MENU_Y));

        boolean consumed = screen.handleContextMenuClick(OUTSIDE_X, OUTSIDE_Y, RIGHT);

        assertTrue(consumed);
        assertFalse(screen.hasOpenPillContextMenu(), "右键点菜单外应关闭 pill 菜单");
    }

    @Test
    void leftClickOutsidePillMenuDoesNotCloseOrConsume() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        assertTrue(screen.openPillContextMenu(pillItem(), MENU_X, MENU_Y));

        boolean consumed = screen.handleContextMenuClick(OUTSIDE_X, OUTSIDE_Y, LEFT);

        assertFalse(consumed, "左键点 pill 菜单外不应被消费");
        assertTrue(screen.hasOpenPillContextMenu(), "左键点菜单外不应关闭 pill 菜单");
    }

    // ---- 无菜单打开时不消费 ----

    @Test
    void noOpenMenuDoesNotConsumeEitherButton() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        assertFalse(screen.handleContextMenuClick(ROW0_X, ROW0_Y, LEFT),
            "无菜单打开时左键不应被消费");
        assertFalse(screen.handleContextMenuClick(ROW0_X, ROW0_Y, RIGHT),
            "无菜单打开时右键不应被消费");
    }

    private static InventoryItem blockItem() {
        return InventoryItem.createFull(
            1L, "earth_crumb", "土块", 1, 1, 0.1, "common", "可放置的土块", 16, 1.0, 1.0);
    }

    private static InventoryItem pillItem() {
        return InventoryItem.createFull(
            9L, "guyuan_pill", "固元丹", 1, 1, 0.2, "rare", "", 1, 1.0, 1.0);
    }
}
