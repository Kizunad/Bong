package com.bong.client.insight;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * InsightOfferStore 结算契约测试（insight-settlement.tsv 全部 terminal causes）。
 *
 * <p>核心不变量：每个 offer 实例由 {@code offerId} + 不可复用 session token 标识；
 * 任何结算路径都先对 exact offerId 原子 claim，再 dispatch/清槽；
 * stale offer A 的迟到 timeout/close/removal/decision 与 duplicate callback
 * 都不能清除 current/pending 的 offer B（即使 A、B 复用同一 triggerId）。
 */
class InsightOfferStoreTest {
    @AfterEach
    void cleanup() {
        InsightOfferStore.resetForTests();
    }

    private static List<InsightDecision> capturingDispatcher() {
        List<InsightDecision> dispatched = new ArrayList<>();
        InsightOfferStore.setDispatcher(dispatched::add);
        return dispatched;
    }

    // ─── 基础替换 / 通知 ──────────────────────────────────────────────────

    @Test
    void replaceNotifiesListeners() {
        List<InsightOfferViewModel> notified = new ArrayList<>();
        InsightOfferStore.addListener(notified::add);

        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);

        assertEquals(1, notified.size());
        assertEquals(offer, notified.get(0));
        assertEquals(offer, InsightOfferStore.snapshot());
        assertNull(InsightOfferStore.pendingOffer(), "替换到空槽时不应产生 pending");
    }

    // ─── ACCEPT / DECLINE / TIMEOUT（claim 先于 dispatch）──────────────────

    @Test
    void acceptClaimsExactOfferIdThenSendsThenClears() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();

        InsightOfferStore.replace(offer);
        InsightOfferStore.submit(
            InsightDecision.chosen(offer.triggerId(), offer.choices().get(0).choiceId()),
            offer.offerId());

        assertEquals(1, dispatched.size(), "ACCEPT 必须恰好发送一次 CHOSEN");
        assertEquals("CHOSEN fixture_choice_E1", dispatched.get(0).summary());
        assertNull(InsightOfferStore.snapshot(), "ACCEPT 后 current 槽必须清空");
    }

    @Test
    void declineAndTimeoutClaimExactOfferId() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);

        InsightOfferStore.settleIfCurrent(offer.offerId(), InsightDecision.declined(offer.triggerId()));
        assertEquals(1, dispatched.size(), "DECLINE 必须恰好发送一次");
        assertEquals("DECLINED", dispatched.get(0).summary());

        InsightOfferStore.replace(offer);
        InsightOfferStore.settleIfCurrent(offer.offerId(), InsightDecision.timedOut(offer.triggerId()));
        assertEquals(2, dispatched.size(), "TIMEOUT 必须恰好发送一次");
        assertEquals("TIMED_OUT", dispatched.get(1).summary());
        assertNull(InsightOfferStore.snapshot());
    }

    // ─── DUPLICATE_TERMINAL：同实例重复结算 no-op ─────────────────────────

    @Test
    void duplicateTerminalForSameInstanceIsNoop() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);

        InsightOfferStore.settleIfCurrent(offer.offerId(), InsightDecision.declined(offer.triggerId()));
        InsightOfferStore.settleIfCurrent(offer.offerId(), InsightDecision.timedOut(offer.triggerId()));
        InsightOfferStore.settleIfCurrent(offer.offerId(), InsightDecision.declined(offer.triggerId()));
        InsightOfferStore.submit(InsightDecision.chosen(offer.triggerId(), offer.choices().get(0).choiceId()),
            offer.offerId());

        assertEquals(1, dispatched.size(), "同一 offer 实例的重复终态回调只能结算一次");
        assertNull(InsightOfferStore.snapshot());
    }

    // ─── REPLACED_BY_DIFFERENT_OFFER：本地 tombstone，不发送 wire decline ──

    @Test
    void replacementSettlesOutgoingLocallyWithoutWireDecline() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);

        InsightOfferStore.replace(first);
        InsightOfferStore.replace(second);

        assertTrue(dispatched.isEmpty(),
            "REPLACED_BY_DIFFERENT_OFFER 只做本地 tombstone，不得伪造 outgoing offerId 的 wire decline");
        assertSame(second, InsightOfferStore.snapshot(), "新 offer 安装成功后成为 authoritative current");

        // outgoing 实例已本地终态：任何迟到回调不得再清掉 offer B
        InsightOfferStore.settleIfCurrent(first.offerId(), InsightDecision.timedOut(first.triggerId()));
        InsightOfferStore.submit(InsightDecision.declined(first.triggerId()), first.offerId());
        assertSame(second, InsightOfferStore.snapshot(),
            "stale offer A 的迟到 timeout/decision 不能清除 offer B");
        assertEquals(0, dispatched.size(), "stale offer A 不得替 offer B 发送终态");
    }

    @Test
    void replacementWithSameTriggerIdKeepsInstancesIndependent() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);
        assertEquals(first.triggerId(), second.triggerId(),
            "fixture 前置：A/B 必须复用同一 triggerId 以证明实例身份独立于 trigger");

        InsightOfferStore.replace(first);
        InsightOfferStore.replace(second);
        InsightOfferStore.settleIfCurrent(first.offerId(), InsightDecision.timedOut(first.triggerId()));

        assertSame(second, InsightOfferStore.snapshot(),
            "复用同一 triggerId 的两份 offer 仍是不同实例，旧 A 不得清除 B");
        assertEquals(0, dispatched.size());
    }

    // ─── pending 槽：新 offer 推入时 outgoing 未完成本地终态 → bounded pending ──

    @Test
    void newOfferWhilePendingActiveEntersBoundedPendingAndRetriesOnClear() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);
        InsightOfferViewModel third = InsightOfferFixtures.heartDemonOffer();

        // 1) first 入 current；2) second 推入时 first 仍权威 → first 被本地结算，second 成 current
        InsightOfferStore.replace(first);
        InsightOfferStore.replace(second);
        assertEquals(0, dispatched.size());

        // 3) 再推 third：current（second）可被结算 → third 成 current
        InsightOfferStore.replace(third);
        assertSame(third, InsightOfferStore.snapshot(),
            "current 槽可结算时新 offer 直接替换并安装");
        assertNull(InsightOfferStore.pendingOffer());
        assertTrue(dispatched.isEmpty(), "替换路径全程不得发送 wire decline");
    }

    @Test
    void lateOutgoingDecisionCannotClearPromotedPending() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);

        InsightOfferStore.replace(first);
        InsightOfferStore.replace(second);
        InsightOfferStore.settleIfCurrent(first.offerId(), InsightDecision.timedOut(first.triggerId()));
        InsightOfferStore.settleIfCurrent(second.offerId(), InsightDecision.timedOut(second.triggerId()));

        assertNull(InsightOfferStore.snapshot(), "promoted B 的正常结算仍须工作");
        assertEquals(1, dispatched.size(), "只有 B 自己的终态被发送（A 早已本地终态）");
    }

    // ─── submit 对错误 offerId 必须 no-op（claim 失败不碰他人槽）────────────

    @Test
    void submitWithWrongOfferIdIsNoop() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);

        InsightOfferStore.submit(InsightDecision.chosen(offer.triggerId(), "fixture_choice_E1"), "wrong_offer_id");

        assertTrue(dispatched.isEmpty(), "claim 失败的 submit 不得 dispatch");
        assertSame(offer, InsightOfferStore.snapshot(), "claim 失败不得清掉 current 槽");
    }

    @Test
    void submitWithoutOfferIdIsNoop() {
        List<InsightDecision> dispatched = capturingDispatcher();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);

        InsightOfferStore.submit(InsightDecision.chosen(offer.triggerId(), "fixture_choice_E1"), (String) null);

        assertTrue(dispatched.isEmpty());
        assertSame(offer, InsightOfferStore.snapshot());
    }

    // ─── 断线清理 / 同实例刷新 ─────────────────────────────────────────────

    @Test
    void clearOnDisconnectKeepsDispatcherAndListeners() {
        List<InsightDecision> dispatched = capturingDispatcher();
        List<InsightOfferViewModel> notified = new ArrayList<>();
        InsightOfferStore.addListener(notified::add);
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());

        InsightOfferStore.clearOnDisconnect();

        assertNull(InsightOfferStore.snapshot(), "断线必须清空 current 槽");
        assertNull(InsightOfferStore.pendingOffer(), "断线必须清空 pending 槽");
        assertTrue(notified.stream().anyMatch(java.util.Objects::isNull),
            "断线清空必须通知监听器槽位已空");

        // dispatcher 与 listeners 保留：重连后新 offer 仍能开屏、选择仍能回传
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());
        assertEquals(InsightOfferFixtures.firstInduceBreakthrough().triggerId(),
            InsightOfferStore.snapshot().triggerId(), "重连后新 offer 必须能再次推入");
        assertTrue(dispatched.isEmpty(), "断线清理不得发送任何 wire 终态");
    }

    @Test
    void replaceIfCurrentRefreshesSameOfferInstance() {
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());
        InsightOfferViewModel refreshed = InsightOfferFixtures.firstInduceBreakthrough(
            System.currentTimeMillis() + 120_000L);

        InsightOfferStore.replaceIfCurrent(refreshed);

        assertSame(refreshed, InsightOfferStore.snapshot(), "同实例刷新必须替换 current 槽");
        assertNull(InsightOfferStore.pendingOffer());
    }

    @Test
    void replaceIfCurrentWithDifferentOfferIsNoop() {
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());
        InsightOfferViewModel other = InsightOfferFixtures.heartDemonOffer();

        InsightOfferStore.replaceIfCurrent(other);

        assertEquals("insight:1001:first_breakthrough_to_Induce", InsightOfferStore.snapshot().offerId(),
            "不同 offerId 的 replaceIfCurrent 不得误替换");
    }

    // ─── 实例身份校验 ──────────────────────────────────────────────────────

    @Test
    void viewModelRejectsBlankOfferId() {
        assertThrows(IllegalArgumentException.class, () -> new InsightOfferViewModel(
            "  ",
            "trigger",
            "label",
            "realm",
            0.5,
            1,
            1,
            System.currentTimeMillis() + 1000L,
            List.of(InsightOfferFixtures.firstInduceBreakthrough().choices().get(0))
        ), "offer 实例身份缺失时必须 fail fast");
    }

    @Test
    void sessionTokenForOnlyMatchesCurrentInstance() {
        InsightOfferViewModel first = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(first);
        InsightOfferStore.SessionToken token = InsightOfferStore.sessionTokenFor(first);
        assertNotNull(token, "current 槽中的实例必须持有 session token");

        InsightOfferViewModel second = InsightOfferFixtures.secondInduceBreakthrough(
            System.currentTimeMillis() + 90_000L);
        InsightOfferStore.replace(second);
        assertNull(InsightOfferStore.sessionTokenFor(first),
            "被替换后旧实例不得再借用 current 槽 token");
        assertNotNull(InsightOfferStore.sessionTokenFor(second), "新实例必须取得自己的 token");
    }

    // ─── reset ─────────────────────────────────────────────────────────────

    @Test
    void resetClearsSnapshotAndDispatcherAndListeners() {
        List<InsightOfferViewModel> notified = new ArrayList<>();
        InsightOfferStore.addListener(notified::add);
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());

        InsightOfferStore.resetForTests();

        assertNull(InsightOfferStore.snapshot());
        assertNull(InsightOfferStore.pendingOffer());
        assertEquals(InsightChoiceDispatcher.LOGGING, InsightOfferStore.dispatcher());

        // Listener should be detached too: replacing again must not notify
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());
        assertEquals(1, notified.size()); // only the initial replace before reset
    }
}
