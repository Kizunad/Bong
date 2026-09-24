package com.bong.client.gathering;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class GatheringPresentationTest {
    @AfterEach
    void reset() {
        GatheringSessionStore.resetForTests();
    }

    @Test
    void progressBlendsConfirmedSnapshotsWithoutPredictingCompletion() {
        GatheringPresentation frame = GatheringPresentation.of(session("a", 20, false, false, 1000))
            .update(session("a", 60, false, false, 1200));
        assertEquals(.2, frame.progress(1200), 1e-6, "收到更新时不能跳帧");
        assertTrue(frame.progress(1275) > .2 && frame.progress(1275) < .6);
        assertEquals(.6, frame.progress(30_000), 1e-6, "等待消息不能推算成采集完成");
        assertTrue(frame.visible(30_000), "活动会话必须等服务端终态再淡出");
    }

    @Test
    void interruptionPreservesVisibleProgressAndDuplicatesCannotRestartExit() {
        GatheringPresentation frame = GatheringPresentation.of(session("a", 20, false, false, 1000))
            .update(session("a", 80, false, false, 1200));
        double visibleProgress = frame.progress(1275);
        frame = frame.update(session("a", 0, true, false, 1275));
        assertEquals(visibleProgress, frame.progress(1500), 1e-6, "中断包进度归零仍应从原弧段散开");
        frame = frame.update(session("a", 0, true, false, 1900));
        assertFalse(frame.visible(2275), "重复终态不能复活旧动画");
        frame = frame.update(session("a", 10, false, false, 2300));
        assertTrue(frame.visible(3300), "伐木复用会话 ID 后仍能开始下一次采集");
        assertEquals(.1, frame.progress(2300), 1e-6);
    }

    @Test
    void completionShowsFullRingOnceAndSessionLifecycleClearsAnimation() {
        GatheringSessionStore.replace(session("a", 80, false, false, 1000));
        GatheringSessionStore.replace(session("a", 100, false, true, 1200));
        GatheringSessionStore.replace(session("a", 100, false, true, 2000));
        assertEquals(1.0, GatheringSessionStore.presentation().progress(1200));
        assertFalse(GatheringSessionStore.presentation().visible(2200), "重复完成包不能延长退场");
        GatheringSessionStore.replace(session("b", 10, false, false, 2300));
        assertEquals(.1, GatheringSessionStore.presentation().progress(2300), 1e-6);
        GatheringSessionStore.clear("a");
        assertTrue(GatheringSessionStore.presentation().visible(2400), "旧会话清理不能影响新采集");
        GatheringSessionStore.clearOnDisconnect();
        assertFalse(GatheringSessionStore.presentation().visible(2400), "断线后不能残留动画");
    }

    private static GatheringSessionViewModel session(String id, long progress, boolean interrupted, boolean completed, long time) {
        return GatheringSessionViewModel.create(id, progress, 100, "草药", "herb", "", "", interrupted, completed, time);
    }
}
