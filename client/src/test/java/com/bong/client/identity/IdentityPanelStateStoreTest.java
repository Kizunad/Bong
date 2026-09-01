package com.bong.client.identity;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

/**
 * plan-bughunt-client-identity-panel-stale-session-v1 — {@link IdentityPanelStateStore#clearOnDisconnect()}
 * 饱和测试。此前该 store 只有 {@code resetForTest()}（测试专用），没有任何生产态清理入口，
 * 导致断线重连后 HUD 角标 / 身份面板会短暂展示上一 session 的身份数据。本测试锁住：
 * ① 清空快照 ② 通知全部监听者（HUD/面板重建依赖这条链路）③ 不误清监听者注册表
 * ④ 不变成一次性开关——reconnect 后正常 replace() 必须继续生效。
 */
final class IdentityPanelStateStoreTest {
    @AfterEach
    void cleanup() {
        IdentityPanelStateStore.resetForTest();
    }

    @Test
    void clearOnDisconnectResetsNonEmptySnapshotToEmpty() {
        IdentityPanelStateStore.replace(new IdentityPanelState(
            2, 500L, 0L,
            List.of(new IdentityPanelEntry(2, "旧局身份", 10, false, List.of()))));
        assertFalse(
            IdentityPanelStateStore.snapshot().identities().isEmpty(),
            "测试前必须模拟断线前残留的非空身份快照，否则无法锁住跨 session 泄漏回归");

        IdentityPanelStateStore.clearOnDisconnect();

        assertEquals(
            IdentityPanelState.empty(),
            IdentityPanelStateStore.snapshot(),
            "断线必须把快照整体复位为 empty()，否则 HUD 角标/身份面板会在新 session 首个 "
                + "identity_panel_state 到达前继续展示上一局身份数据");
    }

    @Test
    void clearOnDisconnectOnAlreadyEmptyStoreStaysEmpty() {
        assertEquals(
            IdentityPanelState.empty(),
            IdentityPanelStateStore.snapshot(),
            "测试前置：store 初始态应为 empty()");

        IdentityPanelStateStore.clearOnDisconnect();

        assertEquals(
            IdentityPanelState.empty(),
            IdentityPanelStateStore.snapshot(),
            "已经是空快照时调用 clearOnDisconnect 应保持 empty()，不应抛异常或产生非法状态");
    }

    @Test
    void clearOnDisconnectNotifiesListenersWithEmptyState() {
        AtomicReference<IdentityPanelState> received = new AtomicReference<>();
        IdentityPanelStateStore.replace(new IdentityPanelState(
            1, 0L, 0L, List.of(new IdentityPanelEntry(1, "旧局身份", 0, false, List.of()))));
        IdentityPanelStateStore.addListener(received::set);

        IdentityPanelStateStore.clearOnDisconnect();

        assertEquals(
            IdentityPanelState.empty(),
            received.get(),
            "clearOnDisconnect 必须像 replace() 一样通知监听者，否则已打开的 IdentityPanelScreen "
                + "收不到清空信号，会继续渲染断线前固化进按钮回调的旧 identityId");
    }

    @Test
    void clearOnDisconnectNotifiesAllRegisteredListeners() {
        AtomicInteger callCount = new AtomicInteger(0);
        IdentityPanelStateStore.addListener(state -> callCount.incrementAndGet());
        IdentityPanelStateStore.addListener(state -> callCount.incrementAndGet());
        IdentityPanelStateStore.addListener(state -> callCount.incrementAndGet());

        IdentityPanelStateStore.clearOnDisconnect();

        assertEquals(
            3,
            callCount.get(),
            "多个订阅者（HUD 角标等）都必须收到断线清空通知");
    }

    @Test
    void clearOnDisconnectDoesNotClearListenerRegistrations() {
        // clearOnDisconnect 只应清快照，不能顺带把监听者列表清空——否则断线时若面板恰好
        // 还没走 removed()，就再也收不到新 session 的通知，永久停在断线前的界面。
        AtomicInteger callCount = new AtomicInteger(0);
        IdentityPanelStateStore.addListener(state -> callCount.incrementAndGet());

        IdentityPanelStateStore.clearOnDisconnect();
        assertEquals(1, callCount.get(), "clearOnDisconnect 本身应触发一次监听者通知");

        IdentityPanelStateStore.replace(IdentityPanelState.empty());
        assertEquals(
            2,
            callCount.get(),
            "clearOnDisconnect 后监听者必须仍然订阅着，后续 replace() 还要继续通知它，"
                + "否则会退化成 resetForTest() 那样的一次性清场（含清监听者）");
    }

    @Test
    void clearOnDisconnectDoesNotBlockNormalReplaceAfterReconnect() {
        IdentityPanelStateStore.replace(new IdentityPanelState(
            9, 0L, 0L, List.of(new IdentityPanelEntry(9, "断线前身份", 0, false, List.of()))));

        IdentityPanelStateStore.clearOnDisconnect();
        assertEquals(
            IdentityPanelState.empty(),
            IdentityPanelStateStore.snapshot(),
            "测试前置：断线后 store 应已复位为空");

        IdentityPanelState newSessionState = new IdentityPanelState(
            5, 200L, 0L, List.of(new IdentityPanelEntry(5, "新局身份", 0, false, List.of())));
        IdentityPanelStateStore.replace(newSessionState);

        assertEquals(
            newSessionState,
            IdentityPanelStateStore.snapshot(),
            "回归防线：clearOnDisconnect 不能变成一次性开关——新 session 收到新 "
                + "identity_panel_state 后正常 replace() 写入必须继续生效");
    }
}
