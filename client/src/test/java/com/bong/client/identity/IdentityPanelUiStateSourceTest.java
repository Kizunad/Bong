package com.bong.client.identity;

import com.bong.client.ui.contract.UiSubscription;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;

final class IdentityPanelUiStateSourceTest {
    @AfterEach
    void cleanup() {
        IdentityPanelStateStore.resetForTest();
    }

    @Test
    void productionSourcePublishesStoreChangesAndStopsAfterClose() {
        IdentityPanelState initial = new IdentityPanelState(
            1, 20L, 3L,
            List.of(new IdentityPanelEntry(1, "旧名", 0, true, List.of())));
        IdentityPanelState next = new IdentityPanelState(
            2, 40L, 0L,
            List.of(new IdentityPanelEntry(2, "当前身份", 12, false, List.of("粗糙"))));
        IdentityPanelStateStore.replace(initial);
        IdentityPanelUiStateSource source = IdentityPanelUiStateSource.production();
        AtomicReference<IdentityPanelState> received = new AtomicReference<>();

        UiSubscription subscription = source.subscribe(received::set);
        IdentityPanelStateStore.replace(next);
        assertEquals(next, source.snapshot(),
            "source 的 snapshot 必须读取当前 Store，而不是缓存订阅首帧");
        assertEquals(next, received.get(), "Store 更新必须通过 source 转发到屏幕");

        subscription.close();
        IdentityPanelStateStore.replace(IdentityPanelState.empty());
        assertEquals(next, received.get(), "关闭订阅后不应再收到迟到的 Store 回调");
    }
}
