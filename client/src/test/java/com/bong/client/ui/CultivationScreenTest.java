package com.bong.client.ui;

import com.bong.client.cultivation.CultivationOverview;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class CultivationScreenTest {
    @AfterEach void cleanup() { PlayerStateStore.resetForTests(); }

    @Test void liveOverviewDropsPreviousPlayerAfterDisconnect() {
        PlayerStateStore.replace(PlayerStateViewModel.create("Spirit", "offline:Azure", 88,100, .45,.82,
            PlayerStateViewModel.PowerBreakdown.empty(),PlayerStateViewModel.SocialSnapshot.empty(),
            "violet_valley","紫霞谷",.91));
        assertFalse(CultivationOverview.describe(PlayerStateStore.snapshot()).placeholder());
        com.bong.client.lifecycle.SessionScopedStoreRegistry.clearAllOnDisconnect();
        assertTrue(CultivationOverview.describe(PlayerStateStore.snapshot()).placeholder(),
            "窗口持续存活时也不能显示旧会话角色的修炼信息");
    }

    @Test void unavailableSnapshotIsNotFabricatedAndNegativePressureRemainsVisible() {
        assertTrue(CultivationOverview.describe(null).placeholder());
        var state=PlayerStateViewModel.create("Solidify", "offline:Azure", 50,100,0,.5,
            PlayerStateViewModel.PowerBreakdown.empty(),PlayerStateViewModel.SocialSnapshot.empty(),
            "rift_mouth_north_001","渊口荒丘",.05,-.8);
        assertTrue(CultivationOverview.describe(state).lines().contains("局部灵压: -0.80"));
    }
}
