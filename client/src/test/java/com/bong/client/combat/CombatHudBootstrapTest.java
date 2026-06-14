package com.bong.client.combat;

import com.bong.client.hud.HudImmersionMode;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
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
        DefenseWindowStore.resetForTests();
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sentPayloads.add(new String(payload, StandardCharsets.UTF_8)));
    }

    @AfterEach
    void reset() {
        HudImmersionMode.resetForTests();
        DefenseWindowStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
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
}
