package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.hud.AlchemyProgressHudPlanner;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lifecycle.SessionScopedStoreRegistry;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.alchemy.AlchemyFurnaceHandler;
import com.bong.client.network.alchemy.AlchemySessionHandler;
import com.bong.client.network.alchemy.AlchemyOutcomeResolvedHandler;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.window.UiWindowManager;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;
import org.junit.jupiter.params.provider.ValueSource;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class AlchemyWindowsTest {
    private static final BlockPos FURNACE = new BlockPos(12, 64, -8);
    private static final UiWindowManager.Rect BOUNDS = new UiWindowManager.Rect(10, 10, 610, 390);
    private final UiWindowManager manager = new UiWindowManager(800, 600);
    private final List<String> sent = new ArrayList<>();
    private AlchemyWindows windows;
    private long now;

    @BeforeEach void setUp() {
        SessionScopedStoreRegistry.clearAllOnDisconnect();
        ClientRequestSender.setBackendForTests((channel, payload) -> sent.add(new String(payload, StandardCharsets.UTF_8)));
        windows = new AlchemyWindows(manager, AlchemyUiStateSource.production(), AlchemyClientIntentSink.production(),
            Runnable::run, ignored -> true, () -> now);
        AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(FURNACE, 2, 88, 100, "旅人", true));
    }

    @AfterEach void tearDown() {
        manager.reset();
        ClientRequestSender.resetBackendForTests();
        SessionScopedStoreRegistry.clearAllOnDisconnect();
    }

    @Test void minimizedWindowTracksRustWireAndTerminalGuidanceWithoutKeepingActiveHud() throws Exception {
        var window = windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_active_v1.pb");
        assertTrue(windows.presentation().active());
        assertEquals("§f44 / 180t", windows.presentation().progressText());
        manager.minimize(window.key());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.inject().kind(), "最小化后不能继续发送操作");
        sessionFixture("alchemy_session_finished_v1.pb");
        furnacePacket(FURNACE, false);
        windows.refresh();
        assertTrue(windows.presentation().terminal(), "收丹后空炉包不能抹掉本炉的终态指导");
        assertTrue(windows.presentation().detailLines().contains("§c× §7t40 (+6) §fdan_sha×3"));
        assertTrue(AlchemyProgressHudPlanner.buildCommands(320, 180, 2000).stream()
            .noneMatch(command -> command.layer() == HudRenderLayer.PROCESSING_HUD), "终态不得继续显示进行中的 HUD");
        manager.restore(window.key());
        assertTrue(windows.presentation().terminal());
        AlchemySessionStore.replace(AlchemySessionStore.Snapshot.empty());
        assertTrue(windows.presentation().idle(), "权威 session reset 才清空终态");
    }

    @Test void incompleteFurnaceSnapshotDoesNotExposeSyncStatusCopy() {
        windows.open(FURNACE, BOUNDS);
        AlchemySessionStore.replace(AlchemySessionStore.Snapshot.empty());

        var presentation = windows.presentation();
        assertEquals("§c炉况异常", presentation.statusText());
        assertEquals("", presentation.progressText());
        assertTrue(presentation.detailLines().isEmpty(), "异常状态由炉底反馈承载，不应伪装成干预面板");
    }

    @Test void closeKeepsFurnaceRunningAndRejectsFurtherActionsUntilReopened() throws Exception {
        var window = windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_active_v1.pb");
        manager.close(window.key());
        assertTrue(sent.isEmpty(), "关闭窗口不是收丹，不得发送结算请求");
        assertTrue(AlchemySessionStore.snapshot().isActive());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.settle().kind());
        windows.open(FURNACE, BOUNDS);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.settle().kind());
        assertEquals(List.of("{\"type\":\"alchemy_take_back\",\"v\":1,\"furnace_pos\":[12,64,-8],\"slot_idx\":0}"), sent,
            "收丹须通过真实 sender 保留既有 wire 契约");
    }

    @ParameterizedTest
    @ValueSource(booleans = {false, true})
    void leavingFurnaceClosesEvenMinimizedWindowWithoutCollecting(boolean minimized) throws Exception {
        var reachable = new java.util.concurrent.atomic.AtomicBoolean(true);
        windows = new AlchemyWindows(manager, AlchemyUiStateSource.production(), AlchemyClientIntentSink.production(),
            Runnable::run, ignored -> reachable.get(), () -> now);
        var window = windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_active_v1.pb");
        if (minimized) manager.minimize(window.key());
        reachable.set(false);
        windows.refresh();
        assertTrue(window.closed(), "离开距离、炉体消失或切维度后，刷新必须主动释放工位");
        assertTrue(AlchemySessionStore.snapshot().isActive(), "自动关闭不能停止服务器正在炼的丹");
        assertTrue(sent.isEmpty());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.inject().kind());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.settle().kind());
        reachable.set(true);
        assertFalse(windows.available(), "走回来不能隐式恢复旧炉授权，须重新交互");
        assertNotSame(window, windows.open(FURNACE, BOUNDS));
    }

    @Test void switchingFurnacesCannotOperateUsingPreviousSession() throws Exception {
        var previous = windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_active_v1.pb");
        BlockPos next = FURNACE.add(3, 0, 0);
        windows.open(next, BOUNDS);
        assertTrue(previous.closed());
        assertFalse(windows.available(), "新的窗口尚未收到同炉授权");
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.inject().kind());
        furnacePacket(next, true);
        windows.refresh();
        assertFalse(windows.presentation().active(), "新炉快照到达时必须清除旧炉 session");
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.inject().kind());
        sessionFixture("alchemy_session_active_v1.pb");
        assertTrue(windows.active());
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.inject().kind());
        assertTrue(sent.get(0).contains("[15,64,-8]"));
    }

    @Test void reopeningWaitsForBothFreshSnapshotsWithoutLosingKnownFurnacePosition() throws Exception {
        windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_active_v1.pb");
        windows.awaitOpenResponse();
        assertFalse(windows.available());
        assertEquals(FURNACE, AlchemyFurnaceStore.snapshot().pos(), "无响应时仍需保留坐标，允许玩家再次交互重试");
        furnacePacket(FURNACE, true);
        windows.refresh();
        assertFalse(windows.active(), "新炉信息不能搭配旧 session 提前恢复操作");
        sessionFixture("alchemy_session_active_v1.pb");
        windows.refresh();
        assertTrue(windows.active());
    }

    @Test void feedUsesLiveInventoryAndNeverCreatesLocalFurnaceItems() {
        var herb = InventoryItem.createFull(31, "ci_she_hao", "刺舌蒿", 1, 1, .2, "common", "", 3, 1, 0);
        var inventory = InventoryModel.builder().gridItem(herb, 0, 0).build();
        InventoryStateStore.replace(inventory);
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, 0, 200,
            .5f, .6f, .1f, 0, 15, "炼制中", List.of(feedStage(0, 0, false)), List.of()));
        windows.open(FURNACE, BOUNDS);
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.feed(null).kind());
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.feed(herb).kind());
        assertSame(inventory, InventoryStateStore.snapshot(), "发包不等于投料成功，不得提前移除背包物品");
        assertFalse(windows.model().session().stages().get(0).completed());
        var water = InventoryItem.createFull(32, "ling_shui", "灵水", 1, 1, .2, "common", "", 1, 1, 0);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(water, 0, 0).build());
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, 1, 200,
            .5f, .6f, .1f, 0, 15, "炼制中", List.of(feedStage(0, 3, false)), List.of()));
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.feed(water).kind(), "同阶段已投过主药，仍应允许继续投入药引");
        InventoryStateStore.replace(InventoryModel.builder().build());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.feed(herb).kind(), "拖动期间已消耗的旧物品不能再次投料");
        assertEquals(2, sent.size());
    }

    @Test void prematureAndLateFeedCannotMarkAStageMissedOnTheServer() {
        var herb = InventoryItem.createFull(31, "ci_she_hao", "刺舌蒿", 1, 1, .2, "common", "", 3, 1, 0);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(herb, 0, 0).build());
        windows.open(FURNACE, BOUNDS);
        for (int tick : List.of(9, 16)) {
            AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, tick, 200,
                .5f, .6f, .1f, 0, 15, "炼制中",
                List.of(feedStage(10, 0, false)), List.of()));
            assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.feed(herb).kind(),
                "不在时间窗口内的投药必须在本地阻止，不能让服务端把该阶段记为错过");
        }
        assertTrue(sent.isEmpty());
    }

    @Test void feedingUsesDraggedStackAndRequiresInventoryAndStageConfirmation() {
        var herb = InventoryItem.createFull(31, "ci_she_hao", "刺舌蒿", 1, 1, .2, "common", "", 1, 1, 0);
        var remainder = InventoryItem.createFull(32, "ci_she_hao", "刺舌蒿", 1, 1, .2, "common", "", 2, 1, 0);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(herb, 0, 0).hotbar(0, remainder).build());
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, 0, 200,
            .5f, .6f, .1f, 0, 15, "炼制中", List.of(feedStage(0, 0, false)), List.of()));
        windows.open(FURNACE, BOUNDS);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.feed(herb).kind());
        assertTrue(sent.get(0).contains("\"count\":1"), "只投入所拖的一份，不能自动合并背包其他堆叠");
        assertNull(windows.confirmedEffect(), "发包不能提前产生投料成功表现");
        InventoryStateStore.replace(InventoryModel.builder().gridItem(herb, 0, 0).hotbar(0, remainder).boneCoins(9).build());
        windows.refresh();
        assertTrue(windows.pending(), "骨币等无关库存变化不能解除投料等待");
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, 6, 200,
            .5f, .6f, .1f, 0, 15, "炼制中", List.of(feedStage(0, 0, true)), List.of()));
        windows.refresh();
        assertTrue(windows.pending(), "阶段变为错过不是投料成功");
        assertNull(windows.confirmedEffect(), "被拒绝或错过时不能冒出成功投料的药烟");
        InventoryStateStore.replace(InventoryModel.builder().hotbar(0, remainder).build());
        windows.refresh();
        assertTrue(windows.pending(), "单独丢弃材料不能冒充投料成功");
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, 0, 200,
            .5f, .6f, .1f, 0, 15, "炼制中", List.of(feedStage(0, 1, false)), List.of()));
        windows.refresh();
        assertFalse(windows.pending(), "材料消耗和阶段已投数量均确认后解除等待");
        var effect = windows.confirmedEffect();
        assertNotNull(effect);
        assertEquals("ci_she_hao", effect.material());
        windows.refresh();
        assertSame(effect, windows.confirmedEffect(), "重复刷新不能再次播放同一次投料");
        assertEquals(1, sent.size());
    }

    @Test void feedingUsesSelectedSplitCountInsteadOfTheAuthoritativeWholeStack() {
        var full = InventoryItem.createFull(41, "ci_she_hao", "刺舌蒿", 1, 1, .2,
            "common", "", 7, 1, 0);
        var selected = full.withStackCount(3);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(full, 0, 0).build());
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true,
            0, 200, .5f, .6f, .1f, 0, 15, "炼制中", List.of(feedStage(0, 0, false)), List.of()));
        windows.open(FURNACE, BOUNDS);

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.feed(selected).kind());
        assertTrue(sent.get(0).contains("\"count\":3"), "中键分出的数量必须成为炼丹投料数量");
    }

    @Test void feedingClampsToRemainingNeedAndRejectsAnAlreadyFullIngredient() {
        var herb = InventoryItem.createFull(31, "ci_she_hao", "刺舌蒿", 1, 1, .2, "common", "", 64, 1, 0);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(herb, 0, 0).build());
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, 0, 200,
            .5f, .6f, .1f, 0, 15, "炼制中", List.of(feedStage(0, 2, false)), List.of()));
        var window = windows.open(FURNACE, BOUNDS);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.feed(herb).kind());
        assertTrue(sent.get(0).contains("\"count\":1"), "尚缺一份时不能投入整堆六十四份");
        windows.acceptMessage("材料已不在背包中");
        assertFalse(windows.pending());
        assertEquals("材料已不在背包中", windows.feedback());
        manager.close(window.key());
        assertFalse(windows.acceptMessage("拒绝"), "丹炉关闭时应保留聊天反馈");
    }

    private static AlchemySessionStore.StageHint feedStage(int tick, int inserted, boolean missed) {
        return new AlchemySessionStore.StageHint(tick, 5, "刺舌蒿×3 + 灵水×1", inserted > 0, missed,
            List.of(new AlchemySessionStore.IngredientHint("ci_she_hao", 3, inserted),
                new AlchemySessionStore.IngredientHint("ling_shui", 1, 0)));
    }

    @Test void placingIncenseUsesItsInstanceAndWaitsForAuthoritativeInventoryConsumption() {
        var incense = InventoryItem.createFull(33, "incense_clear_mind", "静心香", 1, 1, .2,
            "common", "", 1, 1, 0);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(incense, 0, 0).build());
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true, 0, 200,
            .5f, .6f, .1f, 0, 15, "炼制中", List.of(), List.of()));
        windows.open(FURNACE, BOUNDS);

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.placeIncense(incense).kind());
        assertTrue(windows.pending(), "投香发包后应等待服务端扣除实例的快照");
        assertEquals(
            "{\"type\":\"alchemy_place_incense\",\"v\":1,\"furnace_pos\":[12,64,-8],\"item_instance_id\":33}",
            sent.get(0)
        );

        InventoryStateStore.replace(InventoryModel.empty());
        windows.refresh();
        assertTrue(windows.pending(), "仅库存减少不能证明香已投入本炉");
        assertTrue(new AlchemySessionHandler().handle(parse("""
            {"type":"alchemy_session","v":1,"recipe_id":"kai_mai_pill_v0","active":true,
             "elapsed_ticks":1,"target_ticks":200,"temp_current":0.5,"temp_target":0.6,
             "temp_band":0.125,"qi_injected":0,"qi_target":15,"status_label":"炼制中",
             "stages":[],"interventions_recent":[],
             "incense":{"kind":"incense_clear_mind","remaining_ticks":239,"duration_ticks":240,
                        "temp_band_scale":1.25,"qi_cost_scale":1.0,"smoke_color":"#A8D7C5"}}
            """)).handled());
        windows.refresh();
        assertFalse(windows.pending(), "库存扣除与权威香状态同时到达后才确认");
        assertNotNull(windows.confirmedEffect());
        assertEquals("incense", windows.confirmedEffect().material());
    }

    @Test void repeatedConsumptionWaitsForRelevantStateAndTimeoutAllowsRetry() throws Exception {
        windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_active_v1.pb");
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.inject().kind());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.inject().kind());
        sessionFixture("alchemy_session_active_v1.pb");
        windows.refresh();
        assertTrue(windows.pending(), "无关刷新或同值 session 不是注元确认");
        now += 5_001;
        windows.refresh();
        assertFalse(windows.pending(), "无结构化拒绝消息时，等待必须有界");
        assertTrue(windows.feedback().contains("重试"));
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.inject().kind());
        assertEquals(2, sent.size(), "等待期间不能重复消耗真元");
        manager.reset();
        assertFalse(windows.pending(), "断线或窗口清理不得把旧请求带入下一次窗口");
    }

    @Test void finishedSessionIsCollectableButOnlyEmptyFurnaceConfirmsCollection() throws Exception {
        windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_finished_v1.pb");
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.settle().kind());
        windows.refresh();
        assertTrue(windows.pending(), "已到时不代表结果已入袋，不能把旧终态当作新确认");
        furnacePacket(FURNACE, false);
        windows.refresh();
        assertFalse(windows.pending(), "权威空炉状态才确认收取完成");
    }

    @ParameterizedTest
    @ValueSource(booleans = {true, false})
    void resultAnimationWaitsForBothOutcomeAndEmptyFurnace(boolean outcomeFirst) throws Exception {
        windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_finished_v1.pb");
        String recipe = windows.model().session().recipeId();
        outcomePacket("explode", recipe);
        windows.refresh();
        assertNull(windows.confirmedResult(), "浏览炉记不能触发炸炉动画");
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.settle().kind());
        if (outcomeFirst) outcomePacket("explode", recipe);
        else furnacePacket(FURNACE, false);
        windows.refresh();
        assertNull(windows.confirmedResult(), "单个回包或旧炉记不得提前播放结算动画");
        if (outcomeFirst) furnacePacket(FURNACE, false);
        else outcomePacket("explode", recipe);
        windows.refresh();
        assertNull(windows.confirmedResult(), "还须等待本次结束的权威进度，不能拿旧进度猜提前收取");
        sessionFixture("alchemy_session_finished_v1.pb");
        windows.refresh();
        var result = windows.confirmedResult();
        assertNotNull(result, "两种回包顺序都必须确认这一次收取");
        assertEquals(AlchemyResultEffect.EXPLODE, result.effect());
        outcomePacket("explode", recipe);
        windows.refresh();
        assertSame(result, windows.confirmedResult(), "重复快照不能重播炸炉");
        manager.reset();
        windows.open(FURNACE, BOUNDS);
        windows.refresh();
        assertNull(windows.confirmedResult(), "重开窗口不能播放上次炸炉");
    }

    @ParameterizedTest
    @CsvSource({"good,false,COMPLETE", "flawed,false,FLAWED", "waste,false,WASTE", "waste,true,EARLY_TAKE"})
    void collectionOutcomeSelectsItsOwnAnimation(String bucket, boolean early, AlchemyResultEffect expected) {
        windows.open(FURNACE, BOUNDS);
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", true,
            32, 200, .5f, .6f, .1f, 0, 15, "炼制中", List.of(), List.of()));
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.settle().kind());
        outcomePacket(bucket, windows.model().session().recipeId());
        furnacePacket(FURNACE, false);
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("kai_mai_pill_v0", false,
            early ? 32 : 200, 200, .5f, .6f, .1f, 0, 15, "已收取", List.of(), List.of()));
        windows.refresh();
        assertNotNull(windows.confirmedResult());
        assertEquals(expected, windows.confirmedResult().effect(), "提前收取的药渣不得播放炼坏黑烟或炸炉");
    }

    @Test void unrelatedOutcomeAndTimedOutCollectionCannotTriggerExplosion() throws Exception {
        windows.open(FURNACE, BOUNDS);
        sessionFixture("alchemy_session_finished_v1.pb");
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.settle().kind());
        outcomePacket("explode", "another_recipe");
        furnacePacket(FURNACE, false);
        sessionFixture("alchemy_session_finished_v1.pb");
        windows.refresh();
        assertNull(windows.confirmedResult(), "其他丹方的结算不能影响当前炉位");
        now += 5_001;
        windows.refresh();
        outcomePacket("explode", windows.model().session().recipeId());
        windows.refresh();
        assertNull(windows.confirmedResult(), "确认超时后不应把迟到结果配给新的操作");
    }

    private static void outcomePacket(String bucket, String recipe) {
        String json = "{\"type\":\"alchemy_outcome_resolved\",\"v\":1,\"bucket\":\"" + bucket
            + "\",\"recipe_id\":\"" + recipe + "\"}";
        assertTrue(new AlchemyOutcomeResolvedHandler().handle(parse(json)).handled());
    }

    private static void furnacePacket(BlockPos pos, boolean active) {
        String json = "{\"type\":\"alchemy_furnace\",\"v\":1,\"pos_x\":" + pos.getX()
            + ",\"pos_y\":" + pos.getY() + ",\"pos_z\":" + pos.getZ()
            + ",\"integrity\":88,\"has_session\":" + active + "}";
        assertTrue(new AlchemyFurnaceHandler().handle(parse(json)).handled());
    }

    private static void sessionFixture(String name) throws Exception {
        var bridge = ProtoServerDataBridge.bridge(Files.readAllBytes(Path.of("..", "proto", "fixtures", name)));
        assertTrue(bridge.isSuccess(), bridge.errorMessage());
        assertTrue(new AlchemySessionHandler().handle(parse(bridge.legacyJson())).handled());
    }

    private static ServerDataEnvelope parse(String json) {
        var parsed = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(), parsed.errorMessage());
        return parsed.envelope();
    }
}
