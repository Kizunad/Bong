package com.bong.client.combat;

import com.bong.client.hud.BongToast;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.social.SparringInviteScreenBootstrap;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CombatHudBootstrapTest {
    /** Captures every C2S payload the bootstrap dispatches during a test. */
    private final List<String> sentPayloads = new ArrayList<>();

    @BeforeEach
    void installCaptureBackend() {
        sentPayloads.clear();
        CastStateStore.resetForTests();
        DefenseWindowStore.resetForTests();
        QuickUseSlotStore.resetForTests();
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sentPayloads.add(new String(payload, StandardCharsets.UTF_8)));
    }

    @AfterEach
    void reset() {
        CastStateStore.resetForTests();
        DefenseWindowStore.resetForTests();
        QuickUseSlotStore.resetForTests();
        SparringInviteScreenBootstrap.clearOnDisconnect();
        BongToast.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    @Test
    void boundQuickSlotSendsUseRequestWithSameSlot() {
        int slot = QuickSlotConfig.SLOT_COUNT - 1;
        QuickUseSlotStore.replace(QuickSlotConfig.empty().withSlot(slot, new QuickSlotEntry(
            "item:quick-slot-boundary",
            "边界快捷物品",
            750,
            1_500,
            "bong:textures/item/quick_slot_boundary.png"
        )));

        CombatHudBootstrap.onQuickSlotPressed(slot);

        assertEquals(1, sentPayloads.size(),
            "有绑定的快捷槽必须恰好发送一条 use_quick_slot C2S");
        JsonObject payload = JsonParser.parseString(sentPayloads.get(0)).getAsJsonObject();
        assertEquals("use_quick_slot", payload.get("type").getAsString());
        assertEquals(slot, payload.get("slot").getAsInt(),
            "C2S 槽位必须保持与 onQuickSlotPressed 入参一致");
        assertEquals(CastState.Source.QUICK_SLOT, CastStateStore.snapshot().source());
        assertEquals(slot, CastStateStore.snapshot().slot());
    }

    @Test
    void emptyQuickSlotSendsNothingAndKeepsCastIdle() {
        CombatHudBootstrap.onQuickSlotPressed(4);

        assertTrue(sentPayloads.isEmpty(), "空快捷槽不得发送 use_quick_slot C2S");
        assertTrue(CastStateStore.snapshot().isIdle(), "空快捷槽不得启动本地施放状态");
    }

    @Test
    void disconnectAdjunctCleanupResetsBlockedSparringToastDeduplication() {
        String inviteId = "sparring:disconnect-toast";
        notifyBlockedSparringInvite(inviteId);
        assertFalse(BongToast.current(System.currentTimeMillis()).isEmpty(), "旧 session 应先显示邀请提示");

        BongToast.resetForTests();
        CombatHudBootstrap.clearOnDisconnect();
        CombatHudBootstrap.clearOnDisconnect();
        notifyBlockedSparringInvite(inviteId);

        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "adjunct 清理必须复位切磋 blocked-toast 去重状态，使同 ID 在新 session 可再次提示"
        );
    }

    @Test
    void disconnectAdjunctCleanupClearsCombatKeyHeldEdges() {
        CombatKeybindings.setHeldEdgesForTests(true, true);

        CombatHudBootstrap.clearOnDisconnect();

        assertFalse(
            CombatKeybindings.spellVolumeHeldLastTickForTests(),
            "combat bootstrap adjunct cleanup must clear the spell-volume held edge"
        );
        assertFalse(
            CombatKeybindings.shieldHeldLastTickForTests(),
            "combat bootstrap adjunct cleanup must clear the shield held edge"
        );
    }

    @Test
    void disconnectAdjunctCleanupDoesNotClearRegistryOwnedCombatStores() {
        DefenseWindowStore.open(800, 10_000L);
        assertTrue(DefenseWindowStore.snapshot().active(), "前置：防御窗口应处于 active");

        CombatHudBootstrap.clearOnDisconnect();
        CombatHudBootstrap.clearOnDisconnect();

        assertTrue(
            DefenseWindowStore.snapshot().active(),
            "CombatHudBootstrap 只可清非 Store runtime；DefenseWindowStore 必须留给中央 registry"
        );
    }

    @Test
    void sourceLeavesDisconnectRoutingAndStoreClearanceToTheCentralLifecycleOwner() throws Exception {
        String source = Files.readString(Path.of(
            "src/main/java/com/bong/client/combat/CombatHudBootstrap.java"
        ));
        assertTrue(
            !source.contains("ClientPlayConnectionEvents.DISCONNECT.register"),
            "CombatHudBootstrap must not register a distributed DISCONNECT callback"
        );

        assertTrue(
            !source.contains("client.execute("),
            "CombatHudBootstrap must not queue an independently ungated disconnect cleanup task"
        );

        int cleanerStart = source.indexOf("public static void clearOnDisconnect()");
        assertTrue(cleanerStart >= 0, "CombatHudBootstrap must expose a production runtime adjunct cleaner");
        String cleaner = source.substring(cleanerStart);
        assertTrue(
            cleaner.contains("SparringInviteScreenBootstrap.clearOnDisconnect()"),
            "existing sparring runtime UI cleaner must remain owned by CombatHudBootstrap"
        );
        assertTrue(
            !cleaner.contains("Store."),
            "registry-owned Store data must not be cleared by CombatHudBootstrap"
        );
        assertTrue(
            !cleaner.contains("resetForTest"),
            "production adjunct cleaner must not invoke test reset helpers"
        );
        assertTrue(
            !cleaner.contains("clearForTest"),
            "production adjunct cleaner must not invoke test-only clear helpers"
        );
    }


    // ── interaction-intent-cleanup-v1 P3 — 截脉窗口守卫 ────────────────────────

    @Test
    void jiemaiPressWithNoDefenseWindowSendsNothingAndOpensNoRing() {
        // 前置：窗口未开（resetForTests 后 idle）。
        assertFalse(DefenseWindowStore.snapshot().active(),
            "前置应为 idle，否则本用例无法验证「窗口未开」分支");

        CombatHudBootstrap.onJiemaiPressed();

        assertTrue(sentPayloads.isEmpty(),
            "期望窗口未开时按截脉键不发任何 C2S（避免无用 jiemai 洪流），实际发了：" + sentPayloads);
        assertFalse(DefenseWindowStore.snapshot().active(),
            "期望窗口未开时按截脉键不点亮本地截脉环（DefenseWindow 仍 idle），"
                + "实际被本地伪造成 active —— 即旧的 HUD 幻像 bug 回归");
    }

    @Test
    void jiemaiPressDuringActiveWindowSendsJiemaiAndKeepsRingActive() {
        // server 推送防御窗口（截脉机会到来）。
        DefenseWindowStore.open(800, 10_000L);
        assertTrue(DefenseWindowStore.snapshot().active(), "前置：窗口应已开启");

        CombatHudBootstrap.onJiemaiPressed();

        assertEquals(1, sentPayloads.size(),
            "期望窗口开启时按截脉键恰好发一条 C2S，实际发了：" + sentPayloads);
        assertTrue(sentPayloads.get(0).contains("\"jiemai\""),
            "期望发出的是 jiemai 截脉包（type=jiemai），实际 payload=" + sentPayloads.get(0));
        assertTrue(DefenseWindowStore.snapshot().active(),
            "期望按键后本地截脉环维持 active（供 JiemaiRingHudPlanner 渲染），实际被关闭");
    }

    @Test
    void jiemaiPressAfterWindowExpiredIsSuppressed() {
        // 窗口曾开启，但已过期 → tick 关闭后应被守卫挡住（状态转换 active→idle→press）。
        DefenseWindowStore.open(200, 0L);
        assertTrue(DefenseWindowStore.snapshot().active(), "前置：窗口开启");
        DefenseWindowStore.tick(500L); // 越过 expiresAtMs(=200) → 回 idle
        assertFalse(DefenseWindowStore.snapshot().active(), "前置：tick 后窗口应已过期关闭");

        CombatHudBootstrap.onJiemaiPressed();

        assertTrue(sentPayloads.isEmpty(),
            "期望窗口过期后按截脉键不发包，实际发了：" + sentPayloads);
        assertFalse(DefenseWindowStore.snapshot().active(),
            "期望过期窗口不被按键重新点亮，实际被本地伪造成 active");
    }

    @Test
    void repeatedJiemaiPressesAllSuppressedWhileWindowClosed() {
        assertFalse(DefenseWindowStore.snapshot().active(), "前置：窗口未开");

        CombatHudBootstrap.onJiemaiPressed();
        CombatHudBootstrap.onJiemaiPressed();
        CombatHudBootstrap.onJiemaiPressed();

        assertTrue(sentPayloads.isEmpty(),
            "期望窗口未开时连按截脉键依旧零发包（无累积洪流），实际发了：" + sentPayloads);
    }

    private static void notifyBlockedSparringInvite(String inviteId) {
        try {
            var method = SparringInviteScreenBootstrap.class.getDeclaredMethod("notifyBlocked", String.class);
            method.setAccessible(true);
            method.invoke(null, inviteId);
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError("无法驱动切磋邀请 blocked-toast 生产入口", exception);
        }
    }
}
