package com.bong.client.inventory;

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
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-scroll-reading-v1 blocker fix — 玩家侧读卷发起端契约测试。
 *
 * <p>孤岛复盘：server 收端（ScrollOpen emit / handler / store / screen）早已建好，但 client 全仓
 * 无任何触发手段能让玩家发出 {@code ScrollReadRequest}。修复在 {@link InspectScreen} 右键背包物品
 * 的通用 context menu（{@link InspectScreen#availablePillMenuActions}/{@link InspectScreen#triggerPillMenuAction}）
 * 上追加 [阅读] 动作——与既有 [服用]/[研读功法] 同一范式，供玩家右键《经脉浅述·残卷》触发。</p>
 *
 * <p>本类只锁「玩家触发入口 → 发出正确 C2S」这一段契约；server 收到后的处理已在
 * {@code server/src/network/scroll_open_emit.rs} 等测试锁死，不在本类重复覆盖。</p>
 */
class InspectScreenScrollReadTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    private static InventoryItem readableScroll(long instanceId) {
        return InventoryItem.createFull(
            instanceId,
            "scroll_meridian_primer",
            "《经脉浅述·残卷》",
            1, 1, 0.05, "uncommon",
            "残破的手抄卷，勉强能辨认几行经脉浅述。",
            1, 1.0, 1.0
        );
    }

    // ─── happy path：右键菜单命中 + 触发发出正确 C2S ──────────────────

    @Test
    void readableScrollMenuIncludesReadAction() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = readableScroll(4001L);

        assertTrue(screen.openPillContextMenu(item, 10, 20));
        assertTrue(screen.hasOpenPillContextMenu());
        assertEquals(1, screen.availablePillMenuActions(item).size());
        assertEquals("阅读", screen.availablePillMenuActions(item).get(0).label());
        assertEquals(
            InspectScreen.ActionKind.READ_SCROLL,
            screen.availablePillMenuActions(item).get(0).kind()
        );
        assertTrue(sent.isEmpty(), "打开菜单本身不应立即发送 C2S（对齐 openPillContextMenu 既有行为）");
    }

    @Test
    void triggerReadScrollActionSendsScrollReadRequestWithInstanceId() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = readableScroll(4002L);

        assertTrue(screen.openPillContextMenu(item, 10, 20));
        screen.triggerPillMenuAction(InspectScreen.ActionKind.READ_SCROLL);

        assertFalse(screen.hasOpenPillContextMenu(), "触发动作后菜单应关闭（对齐既有 triggerPillMenuAction 行为）");
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"scroll_read_request\",\"v\":1,\"instance_id\":4002}",
            sent.get(0).body(),
            "wire 形状必须匹配 server ClientRequestV1::ScrollReadRequest{v:1,instance_id:u64}"
        );
    }

    @Test
    void dispatchScrollReadRequestSendsForKnownReadableScroll() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = readableScroll(4003L);

        assertTrue(screen.dispatchScrollReadRequest(item));
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"scroll_read_request\",\"v\":1,\"instance_id\":4003}",
            sent.get(0).body()
        );
    }

    // ─── 边界 / 错误分支：不该发送的场景一律不发 ──────────────────────

    @Test
    void combatTechniqueScrollMenuHasNoReadActionOnlyStudyAction() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        // 消耗式招式残卷：有 scrollKind（走 TECHNIQUE_SCROLL_USE「研读功法」），不是
        // readable_scroll_spec 挂载物（item_id 未注册进 ScrollVanillaIconMap）——不应额外出现 [阅读]。
        InventoryItem item = InventoryItem.createFullWithScrollMeta(
            4004L,
            "scroll_woliu_vortex",
            "涡流残卷·绝灵涡流",
            1, 2, 0.05, "uncommon", "泛黄残页", 1, 1.0, 1.0,
            "combat_technique", "woliu.vortex", 0
        );

        List<InspectScreen.PillMenuAction> actions = screen.availablePillMenuActions(item);
        assertEquals(1, actions.size(), "combat technique scroll must only offer 研读功法, not 阅读: " + actions);
        assertEquals("研读功法", actions.get(0).label());
        assertTrue(
            actions.stream().noneMatch(a -> a.kind() == InspectScreen.ActionKind.READ_SCROLL),
            "combat technique scroll (readable_scroll_spec-less) must never expose READ_SCROLL action"
        );
    }

    @Test
    void plainInventoryItemMenuHasNoReadAction() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            4005L, "spirit_wood", "灵木", 1, 2, 1.2, "common",
            "真元载体。", 1, 0.8, 1.0
        );

        assertTrue(screen.availablePillMenuActions(item).isEmpty());
        assertFalse(screen.openPillContextMenu(item, 10, 20));
    }

    @Test
    void dispatchScrollReadRequestRejectsUnknownTemplateId() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            4006L, "scroll_meridian_primer_v2_unregistered", "未来某个新残卷", 1, 1, 0.05,
            "uncommon", "尚未接入 ScrollVanillaIconMap 的假想残卷", 1, 1.0, 1.0
        );

        assertFalse(screen.dispatchScrollReadRequest(item));
        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchScrollReadRequestRejectsZeroInstanceId() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        // instanceId=0L 代表 mock/未持有权威实例（对齐 dispatchApplyPillSelf 的同类边界）。
        InventoryItem item = readableScroll(0L);

        assertFalse(screen.dispatchScrollReadRequest(item));
        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchScrollReadRequestRejectsNullItem() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        assertFalse(screen.dispatchScrollReadRequest(null));
        assertTrue(sent.isEmpty());
    }
}
