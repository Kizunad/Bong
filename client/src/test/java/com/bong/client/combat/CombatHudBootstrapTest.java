package com.bong.client.combat;

import com.bong.client.hud.HudImmersionMode;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.AnqiHudState;
import com.bong.client.hud.AnqiHudStateStore;
import com.bong.client.combat.handler.AnqiHudServerDataHandler;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import com.bong.client.social.SparringInviteScreenBootstrap;
import com.bong.client.social.SocialStateStore;
import com.bong.client.state.VisualEffectState;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
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
        AnqiHudStateStore.clear();
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sentPayloads.add(new String(payload, StandardCharsets.UTF_8)));
    }

    @AfterEach
    void reset() {
        CastStateStore.resetForTests();
        HudImmersionMode.resetForTests();
        DefenseWindowStore.resetForTests();
        QuickUseSlotStore.resetForTests();
        AnqiHudStateStore.clear();
        SocialStateStore.resetForTests();
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
    void resetOnDisconnectClearsHudImmersionCombatWindow() {
        CombatHudState combat = CombatHudState.create(0.8f, 0.7f, 0.4f, DerivedAttrFlags.none());
        assertEquals(
            HudImmersionMode.Mode.COMBAT,
            HudImmersionMode.resolve(combat, VisualEffectState.none(), 1_000L)
        );

        CombatHudBootstrap.resetOnDisconnect();

        assertEquals(
            HudImmersionMode.Mode.PEACE,
            HudImmersionMode.resolve(CombatHudState.empty(), VisualEffectState.none(), 1_500L)
        );
    }

    @Test
    void resetOnDisconnectOpensNewTickEpochForAllProducedAnqiHudDimensions() {
        long now = System.currentTimeMillis();
        long oldSessionTick = 72_000L;
        long newSessionTick = 10L;

        AnqiHudStateStore.updateEcho(8, now, 2_000L, oldSessionTick);
        AnqiHudStateStore.updateCharge(0.8f, now, 2_000L, oldSessionTick);
        AnqiHudStateStore.updateAbrasion("quiver", 80.0f, now, 2_000L, oldSessionTick);
        AnqiHudStateStore.updateMultiShot(8, now, 2_000L, oldSessionTick);

        CombatHudBootstrap.resetOnDisconnect();

        assertEquals(AnqiHudState.empty(), AnqiHudStateStore.snapshot(now),
            "生产断线 reset 必须先清空旧 session 的暗器 HUD 快照");

        AnqiHudStateStore.updateEcho(2, now, 2_000L, newSessionTick);
        AnqiHudStateStore.updateCharge(0.2f, now, 2_000L, newSessionTick);
        AnqiHudStateStore.updateAbrasion("hand_slot", 20.0f, now, 2_000L, newSessionTick);
        AnqiHudStateStore.updateMultiShot(2, now, 2_000L, newSessionTick);

        AnqiHudState state = AnqiHudStateStore.snapshot(now);
        assertEquals(2, state.echoCount(), "新 session 的低 tick echo 必须被接受");
        assertEquals(0.2f, state.chargeProgress(), 0.001f,
            "新 session 的低 tick charge 必须被接受");
        assertEquals("hand_slot", state.abrasionContainer(),
            "新 session 的低 tick abrasion 必须被接受");
        assertEquals(20.0f, state.abrasionQiPayload(), 0.001f);
        assertEquals(2, state.multiShotCount(),
            "新 session 的低 tick multishot 必须被接受");
    }

    @Test
    void resetOnDisconnectLetsRealHandlerAcceptLowerTickFromNewSession() {
        long now = System.currentTimeMillis();
        AnqiHudStateStore.updateEcho(9, now, 2_000L, 72_000L);

        CombatHudBootstrap.resetOnDisconnect();

        String payload = "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"echo\","
            + "\"echo_count\":3,\"aim_progress\":0.0,\"charge_progress\":0.0,"
            + "\"abrasion_container\":\"\",\"abrasion_qi_payload\":0.0,\"tick\":10}";
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            payload, payload.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(), "测试 payload 必须先通过真实 envelope parser");

        ServerDataDispatch dispatch = new AnqiHudServerDataHandler().handle(parsed.envelope());

        assertTrue(dispatch.handled(), "新 session 的低 tick anqi_hud 应被真实 handler 消费");
        assertEquals(3, AnqiHudStateStore.snapshot().echoCount(),
            "disconnect reset 后 handler 不得把新 session 低 tick 当成旧包静默丢弃");
    }

    @Test
    void resetOnDisconnectClearsEntireSparringInviteLifecycle() {
        SocialStateStore.SparringInvite previous = invite("sparring:0002", 6_000L);
        assertEquals(SocialStateStore.SparringInviteUpdate.ACCEPTED, SocialStateStore.enqueueSparringInvite(previous));
        SocialStateStore.clearSparringInvite(previous.inviteId());
        assertEquals(
            SocialStateStore.SparringInviteUpdate.SETTLED,
            SocialStateStore.enqueueSparringInvite(previous),
            "测试前置：旧 session 应已留下 settled tombstone"
        );
        assertEquals(
            SocialStateStore.SparringInviteUpdate.ACCEPTED,
            SocialStateStore.enqueueSparringInvite(invite("sparring:0003", 7_000L))
        );

        CombatHudBootstrap.resetOnDisconnect();

        assertNull(SocialStateStore.sparringInvite(), "生产断线入口必须清空旧 session 的 pending 邀请");
        assertEquals(
            SocialStateStore.SparringInviteUpdate.ACCEPTED,
            SocialStateStore.enqueueSparringInvite(previous),
            "新 session 必须同时复位 tombstone 与版本高水位，不能拒绝合法复用 identity"
        );
    }

    @Test
    void resetOnDisconnectClearsBlockedSparringToastDeduplication() {
        String inviteId = "sparring:disconnect-toast";
        notifyBlockedSparringInvite(inviteId);
        assertFalse(BongToast.current(System.currentTimeMillis()).isEmpty(), "旧 session 应先显示邀请提示");

        BongToast.resetForTests();
        CombatHudBootstrap.resetOnDisconnect();
        notifyBlockedSparringInvite(inviteId);

        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "生产断线入口必须复位 blocked-toast 去重状态，使新 session 的同 ID 邀请重新提示"
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

    private static SocialStateStore.SparringInvite invite(String inviteId, long expiresAtMs) {
        return new SocialStateStore.SparringInvite(
            inviteId,
            "char:a",
            "char:b",
            "凝脉",
            "气息相试",
            "点到为止",
            expiresAtMs
        );
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
