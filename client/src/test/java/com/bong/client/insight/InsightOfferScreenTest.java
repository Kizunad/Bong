package com.bong.client.insight;

import com.bong.client.network.ClientRequestSender;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * InsightOfferScreen 结算生命周期测试（r7-insight-settlement.tsv ESC / TIMEOUT /
 * ANIMATED_OPEN_CANCELLED / REMOVED_EXCEPTIONALLY / REPLACED_BY_DIFFERENT_OFFER）。
 *
 * <p>结算路径恒为：exact offerId claim 先于 dispatch 先于 close；被替换后旧屏的迟到
 * close/tick/转场回调都不能清除后来的 offer B（即使 A/B 复用同一 triggerId）。
 */
class InsightOfferScreenTest {
    private static final String CLIENT_REQUEST = "bong:client_request";

    private final List<String> sentPayloads = new ArrayList<>();
    private final AtomicLong clock = new AtomicLong(1_000_000L);

    @AfterEach
    void cleanup() {
        InsightOfferStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    /** 绑定真实生产 dispatcher 链路（claim 后经 ClientRequestInsightDispatcher 编码 C2S payload）。 */
    private void bindWireBackend() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8)));
        InsightOfferStore.setDispatcher(new ClientRequestInsightDispatcher());
    }

    // ─── ACCEPT（点击候选卡，由 Store::submit 回传）──────────────────────────

    @Test
    void chooseSendsChosenAndSettlesCurrentOffer() {
        bindWireBackend();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);
        InsightOfferScreen screen = new InsightOfferScreen(offer);

        screen.settleForTests(InsightDecision.chosen(offer.triggerId(), offer.choices().get(0).choiceId()));

        assertEquals(1, sentPayloads.size(), "CHOSEN 必须恰好发送一次");
        assertTrue(sentPayloads.get(0).contains("first_breakthrough_to_Induce"),
            "C2S 决定仍只带 triggerId（wire 无 offerId 关联），payload=" + sentPayloads.get(0));
        assertNull(InsightOfferStore.snapshot(), "ACCEPT 后 current 槽必须清空");
    }

    // ─── ESC（close 收敛为 declined，claim 先于 dispatch）───────────────────

    @Test
    void escSettlesDeclinedExactlyOnce() {
        bindWireBackend();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);
        InsightOfferScreen screen = new InsightOfferScreen(offer);

        screen.closeForTests();
        screen.closeForTests();
        screen.tickForTests();

        assertEquals(1, sentPayloads.size(), "重复 ESC 必须恰好发送一条 DECLINED");
        assertTrue(
            sentPayloads.get(0).contains("\"type\":\"insight_decision\"")
                && sentPayloads.get(0).contains("\"choice_idx\":null")
                && sentPayloads.get(0).contains("\"trigger_id\":\"first_breakthrough_to_Induce\""),
            "ESC 必须按 declined 语义结算（C2S insight_decision 只带 trigger_id，choice_idx 为 null），payload="
                + sentPayloads.get(0));
        assertNull(InsightOfferStore.snapshot(), "ESC 后 current 槽必须清空");
    }

    // ─── TIMEOUT（tick 过期 → TIMED_OUT）────────────────────────────────────

    @Test
    void expiredTickSettlesTimedOutExactlyOnce() {
        bindWireBackend();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough(1_000_000L);
        InsightOfferStore.replace(offer);
        InsightOfferScreen screen = new InsightOfferScreen(offer, clock::get);

        clock.set(1_000_001L); // 过期
        screen.tickForTests();
        screen.tickForTests();
        screen.closeForTests();

        assertEquals(1, sentPayloads.size(), "TIMEOUT 后重复 tick/close 不得再发送");
        assertNull(InsightOfferStore.snapshot(), "过期 offer 结算后槽位必须清空");
    }

    // ─── REPLACED_BY_DIFFERENT_OFFER：旧屏迟到回调不能清新 offer ─────────────

    @Test
    void staleScreenTimeoutCannotClearReplacementOffer() {
        bindWireBackend();
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);
        InsightOfferStore.replace(first);
        InsightOfferScreen staleScreen = new InsightOfferScreen(first);
        InsightOfferStore.replace(second); // 旧 offer 被本地结算，第二份成为 current

        staleScreen.tickForTests();        // 迟到 timeout
        staleScreen.closeForTests();       // 迟到 ESC

        assertSame(second, InsightOfferStore.snapshot(),
            "stale offer A 的迟到 timeout/close 不能清除 offer B");
        assertTrue(sentPayloads.isEmpty(),
            "stale offer A 不得替 offer B 发送终态，实际=" + sentPayloads);
    }

    @Test
    void staleScreenWithSameTriggerIdCannotClearNewInstance() {
        bindWireBackend();
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);
        assertEquals(first.triggerId(), second.triggerId(),
            "前置：A/B 复用同一 triggerId——实例身份必须独立于 trigger");
        InsightOfferStore.replace(first);
        InsightOfferScreen staleScreen = new InsightOfferScreen(first);
        InsightOfferStore.replace(second);

        staleScreen.closeForTests();
        staleScreen.onPendingOpenCancelledForTests();
        staleScreen.onCurrentScreenCancelledForTests();

        assertSame(second, InsightOfferStore.snapshot(),
            "复用同一 triggerId 的迟到回调仍不能清除新实例");
        assertTrue(sentPayloads.isEmpty());
    }

    // ─── ANIMATED_OPEN_CANCELLED / REMOVED_EXCEPTIONALLY ───────────────────

    @Test
    void pendingOpenCancellationSettlesDeclinedExactlyOnce() {
        bindWireBackend();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);
        InsightOfferScreen screen = new InsightOfferScreen(offer);

        screen.onPendingOpenCancelledForTests();
        screen.onPendingOpenCancelledForTests();
        screen.closeForTests();

        assertEquals(1, sentPayloads.size(), "ANIMATED_OPEN_CANCELLED 后重复回调不得再发送");
        assertNull(InsightOfferStore.snapshot(), "开屏转场取消后槽位必须清空");
    }

    @Test
    void currentScreenCancellationSettlesDeclinedExactlyOnce() {
        bindWireBackend();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);
        InsightOfferScreen screen = new InsightOfferScreen(offer);

        screen.onCurrentScreenCancelledForTests();
        screen.onCurrentScreenCancelledForTests();
        screen.removedForTests();

        assertEquals(1, sentPayloads.size(), "REMOVED_EXCEPTIONALLY 后重复回调不得再发送");
        assertNull(InsightOfferStore.snapshot());
    }

    @Test
    void exceptionalRemovalSettlesDeclinedExactlyOnce() {
        bindWireBackend();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);
        InsightOfferScreen screen = new InsightOfferScreen(offer);

        screen.removedForTests();
        screen.removedForTests();

        assertEquals(1, sentPayloads.size(), "异常移除只能发送一条 declined");
        assertNull(InsightOfferStore.snapshot());
    }

    // ─── 转场仲裁：同 token 延续 vs 新实例覆盖 ─────────────────────────────

    @Test
    void sameInstanceRefreshContinuesWithOwnScreen() {
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);
        InsightOfferScreen firstScreen = new InsightOfferScreen(offer);
        InsightOfferStore.replaceIfCurrent(offer); // 同实例刷新保留 token

        InsightOfferScreen secondScreen = new InsightOfferScreen(offer);

        assertTrue(firstScreen.continuesWithForTests(secondScreen),
            "同实例的新屏必须延续旧屏的 session（同 token）");
    }

    @Test
    void differentOfferReplacementDoesNotContinueWithOldScreen() {
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);
        InsightOfferStore.replace(first);
        InsightOfferScreen firstScreen = new InsightOfferScreen(first);
        InsightOfferStore.replace(second);

        InsightOfferScreen secondScreen = new InsightOfferScreen(second);

        assertTrue(!firstScreen.continuesWithForTests(secondScreen),
            "不同 offerId 的新屏不得与旧屏共享会话 token");
    }
}
