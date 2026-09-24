package com.bong.client.craft;

import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class CraftWindowsTest {
    private final UiWindowManager manager = new UiWindowManager(800, 600);
    private final List<CraftIntent> sent = new ArrayList<>();
    private final UiWindowManager.Rect bounds = new UiWindowManager.Rect(0, 0, 650, 390);
    private final CraftContext bench = new CraftContext(new CraftContext.Workbench(7, 1, 2, 3));
    private boolean available = true;
    private long now;
    private CraftWindows windows;

    @BeforeEach
    void setup() {
        CraftStore.clear();
        InventoryStateStore.clearOnDisconnect();
        CraftStore.replaceRecipes(List.of(recipe("hand", null), recipe("bench", "workbench"), recipe("forge", "forge")));
        windows = new CraftWindows(manager, CraftUiStateSource.production(), intent -> {
            sent.add(intent);
            return UiIntentResult.accepted();
        }, Runnable::run, ignored -> available, () -> now);
    }

    @AfterEach
    void cleanup() {
        manager.reset();
        CraftStore.clear();
        InventoryStateStore.clearOnDisconnect();
    }

    @Test
    void onlyStagedMaterialsEnableCraftAndClosingReturnsThemBeforeAnotherRecipeCanStart() {
        var item = InventoryItem.createFull(41L, "iron", "铁片", 1, 1, 0.2, "common", "", 2, 1.0, 0.4);
        CraftStore.replaceRecipes(List.of(new CraftRecipe("hand", CraftCategory.MISC, "工具",
            List.of(new CraftRecipe.MaterialEntry("iron", 2)), 0, 80, "output", 1,
            CraftRecipe.Requirements.NONE, true)));
        InventoryStateStore.applyAuthoritativeSnapshot(InventoryModel.builder().gridItem(item, 0, 0).build(), 7);
        var state = windows.open(CraftContext.HANDCRAFT, bounds);
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start("hand", 1).kind(),
            "背包有材料不代表已放入制作区");
        windows.material("hand", item.instanceId(), false, windows.model().inventoryRevision());
        assertEquals(List.of(new CraftIntent.Material("hand", 41L, false, 7)), sent);
        assertTrue(windows.busy(), "移入请求必须等待服务端库存回执");

        InventoryStateStore.applyAuthoritativeSnapshot(InventoryModel.builder()
            .craftPreparation("hand", List.of(item)).build(), 8);
        assertFalse(windows.busy());
        windows.close(state);
        assertEquals(new CraftIntent.Cancel(), sent.get(1), "未开工关闭也必须请求完整返还");
        windows.open(bench, bounds);
        assertEquals(CraftContext.HANDCRAFT, windows.context());
        assertTrue(windows.busy(), "返还尚未确认时不能复用旧材料");
        InventoryStateStore.applyAuthoritativeSnapshot(InventoryModel.builder().gridItem(item, 0, 0).build(), 9);
        assertFalse(windows.busy());
        windows.open(bench, bounds);
        assertEquals(bench, windows.context());
    }

    @Test
    void minimizingAndPinningRetainTheSingleAuthoritativeSessionUntilExplicitClose() {
        var state = windows.open(CraftContext.HANDCRAFT, bounds);
        windows.start("hand", 1);
        CraftStore.replaceSession(new CraftSessionStateView(true, "hand", 1, 80));
        manager.minimize(state.key());
        manager.pin(state.key(), true);
        manager.tick(1000);
        CraftStore.replaceSession(new CraftSessionStateView(true, "hand", 40, 80));
        assertEquals(40, windows.model().session().elapsedTicks(), "隐藏后仍必须接收服务端进度");
        assertSame(state, windows.open(bench, bounds));
        assertEquals(CraftContext.HANDCRAFT, windows.context(), "制作中不能把旧会话冒充另一工位");
        assertEquals(1, manager.snapshot().size());
        assertEquals(List.of(new CraftIntent.Start("hand", 1)), sent, "隐藏和切入口不能自动取消");

        windows.close(state);
        windows.close(state);
        assertEquals(List.of(new CraftIntent.Start("hand", 1), new CraftIntent.Cancel()), sent);
        assertTrue(manager.snapshot().isEmpty());
        windows.open(bench, bounds);
        assertEquals(CraftContext.HANDCRAFT, windows.context(), "取消回执到达前，重开也不能重新标记旧会话");
    }

    @Test
    void pendingStartBlocksDoubleSubmissionAndCloseCancelsItBeforeAcknowledgment() {
        var oldWindow = windows.open(CraftContext.HANDCRAFT, bounds);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.start("hand", 1).kind());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start("hand", 1).kind());
        windows.open(bench, bounds);
        assertEquals(CraftContext.HANDCRAFT, windows.context());
        windows.close(oldWindow);
        var reopened = windows.open(bench, bounds);
        windows.close(oldWindow);
        assertTrue(manager.contains(reopened.key()), "迟到的旧关闭回调不能关闭新窗口");
        assertEquals(List.of(new CraftIntent.Start("hand", 1), new CraftIntent.Cancel()), sent);
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start("hand", 1).kind(),
            "关闭后立即重开必须等待取消结算，避免开始/取消事件队列交叉");
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.failed("hand", "player_cancelled", 0, 0));
        windows.open(bench, bounds);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.start("bench", 1).kind());
    }

    @Test
    void onlyIdleSessionsCanChangeStationsAndInvalidTargetsCannotStart() {
        windows.open(bench, bounds);
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start("hand", 1).kind());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start("forge", 1).kind());
        available = false;
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start("bench", 1).kind());
        assertTrue(sent.isEmpty(), "配方范围和失效工位必须在发送前拦截");
        available = true;
        windows.start("bench", 1);
        CraftStore.replaceSession(new CraftSessionStateView(true, "bench", 1, 80));
        var other = new CraftContext(new CraftContext.Workbench(8, 9, 2, 3));
        windows.open(other, bounds);
        assertEquals(bench, windows.context());
        CraftStore.replaceSession(CraftSessionStateView.IDLE);
        windows.open(other, bounds);
        assertEquals(other, windows.context(), "服务端确认结束后才切换工位");
    }

    @Test
    void disconnectCleansSubscriptionsWithoutSendingCancelIntoAnotherConnection() {
        windows.open(CraftContext.HANDCRAFT, bounds);
        List<CraftScreenViewModel> updates = new ArrayList<>();
        windows.listen(updates::add);
        windows.start("hand", 1);
        manager.reset();
        int count = updates.size();
        CraftStore.replaceSession(new CraftSessionStateView(true, "hand", 5, 80));
        assertEquals(count, updates.size(), "连接重置后不能再更新已关闭视图");
        assertEquals(List.of(new CraftIntent.Start("hand", 1)), sent, "断线清理不是业务取消");
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start("hand", 1).kind());
    }

    @Test
    void reopeningHiddenWindowDoesNotDuplicateOutcomeFeedback() {
        var state = windows.open(CraftContext.HANDCRAFT, bounds);
        List<String> feedback = new ArrayList<>();
        windows.listen(model -> {
            if (model.change() == CraftScreenViewModel.Change.OUTCOME) {
                CraftOutcomeFeedback.apply(model, ticks -> feedback.add("flash"),
                    () -> feedback.add("sound"), () -> feedback.add("refresh"));
            }
        });
        manager.minimize(state.key());
        windows.open(CraftContext.HANDCRAFT, bounds);
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed("hand", "output", 1, 80));
        assertEquals(List.of("flash", "sound", "refresh"), feedback);
        windows.close(state);
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed("hand", "output", 1, 160));
        assertEquals(3, feedback.size(), "关闭后不能重播完成反馈");
    }

    private static CraftRecipe recipe(String id, String station) {
        return new CraftRecipe(id, CraftCategory.MISC, id, List.of(), 0, 80, "output", 1,
            CraftRecipe.Requirements.NONE, true, station);
    }

    @Test
    void missingCraftReplyAllowsRetryButTimeoutNeverOverridesAnActiveServerSession() {
        var state = windows.open(CraftContext.HANDCRAFT, bounds);
        windows.start("hand", 1);
        now = 60_000;
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.start("hand", 1).kind(),
            "通用 gate 拒绝没有 craft 回执时，等待超时后必须能重试");
        windows.close(state);
        windows.open(CraftContext.HANDCRAFT, bounds);
        assertTrue(windows.busy());
        now += 60_000;
        assertFalse(windows.busy(), "取消无回执也不能把重新打开的窗口永久锁住");
        CraftStore.replaceSession(new CraftSessionStateView(true, "hand", 1, 80));
        now += 60_000;
        assertTrue(windows.busy(), "权威 active 会话永远不由本地超时清除");
    }
}
