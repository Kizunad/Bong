package com.bong.client.forge;

import com.bong.client.forge.state.*;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.forge.ForgeOutcomeHandler;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.state.StoreUiStateSource;
import com.bong.client.ui.window.UiWindowManager;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import java.util.ArrayList;
import java.util.List;
import static org.junit.jupiter.api.Assertions.*;

class ForgeWindowsTest {
    private final UiWindowManager manager = new UiWindowManager(800, 600);
    private final List<ForgeIntent> sent = new ArrayList<>();
    private final BlockPos pos = new BlockPos(2, 64, 0);
    private final UiWindowManager.Rect bounds = new UiWindowManager.Rect(20, 20, 400, 350);
    private boolean reachable = true;
    private long now;
    private ForgeWindows windows;

    @BeforeEach void setup() {
        ForgeStationStore.replace(new ForgeStationStore.Snapshot(pos, "station", 2, 1, "owner", false));
        BlueprintScrollStore.replace(List.of(new BlueprintScrollStore.Entry("iron_sword_v0", "铁剑", 1, 1,
            "iron_sword", List.of("billet"), List.of(new BlueprintScrollStore.Material("fan_tie", 3)))), 0);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(material(1, 2), 0, 0)
            .gridItem(material(2, 3), 0, 1).build());
        windows = new ForgeWindows(manager, StoreUiStateSource.pullOnOpen(ForgeViewModel::snapshot), intent -> {
            sent.add(intent);
            return UiIntentResult.accepted();
        }, ignored -> reachable, () -> now);
    }

    @AfterEach void cleanup() {
        manager.reset();
        ForgeStationStore.clearOnDisconnect();
        ForgeSessionStore.clearOnDisconnect();
        ForgeOutcomeStore.clearOnDisconnect();
        BlueprintScrollStore.clearOnDisconnect();
        InventoryStateStore.clearOnDisconnect();
    }

    @Test void startOnlyConsumesAuthoritativePreparedMaterialsAndWaitsForAuthority() {
        windows.open(pos, bounds);
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start().kind(), "背包物品没有入炉不能开炉");
        prepare(2, 1);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.start().kind());
        assertEquals(List.of(new ForgeIntent.Start(pos, "iron_sword_v0",
            List.of(new ClientRequestProtocol.ForgeMaterial("fan_tie", 3)))), sent,
            "多堆入炉矿物必须按 canonical id 汇总，不能从背包补料");
        assertFalse(windows.model().session().active(), "发送成功不代表服务端已经开炉");
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start().kind());
        now = 6000;
        windows.refresh();
        assertFalse(windows.pending(), "拒绝没有专属回执时允许超时重试");
        InventoryStateStore.replace(InventoryModel.empty());
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.start().kind(), "发送前重读材料，不能用过时快照开炉");
    }

    @Test void stationWindowMovesAndMinimizesButNeverPinsOrCancelsTheServerSession() {
        var state = windows.open(pos, bounds);
        assertTrue(manager.beginDrag(state.key(), 22, 22));
        assertTrue(manager.dragTo(50, 50));
        assertFalse(manager.pin(state.key(), true));
        manager.minimize(state.key());
        assertSame(state, windows.open(pos, bounds));
        manager.close(state.key());
        assertTrue(sent.isEmpty(), "关闭或最小化不能自动取消炉次");
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.advance().kind());
    }

    @Test void injectionStopsOnReleaseFocusLossMinimizeAndDisconnect() {
        ForgeSessionStore.replace(session(7, "consecration", "{\"qi_injected\":0,\"qi_required\":80}"));
        var state = windows.open(pos, bounds);
        windows.beginInjection();
        windows.tickInjection(true);
        assertEquals(List.of(new ForgeIntent.Inject(7, 2.5)), sent);
        windows.tickInjection(true);
        assertEquals(1, sent.size(), "同一回执前不能重复注入");
        ForgeSessionStore.replace(session(7, "consecration", "{\"qi_injected\":2.5,\"qi_required\":80}"));
        windows.refresh();
        windows.endInjection();
        windows.tickInjection(true);
        windows.beginInjection();
        windows.tickInjection(false);
        windows.tickInjection(true);
        windows.beginInjection();
        manager.minimize(state.key());
        windows.tickInjection(true);
        windows.open(pos, bounds);
        windows.tickInjection(true);
        windows.beginInjection();
        manager.reset();
        windows.tickInjection(true);
        assertEquals(1, sent.size(), "任何输入所有权丢失都必须停止持续注入");
    }

    @Test void outcomeEndsOnlyTheMatchingSessionAndReenablesPreparation() {
        ForgeSessionStore.replace(session(7, "billet", "{}"));
        windows.open(pos, bounds);
        applyOutcome(6);
        assertTrue(ForgeSessionStore.snapshot().active(), "旧炉次结果不能结束当前炉次");
        applyOutcome(7);
        windows.refresh();
        assertFalse(windows.model().session().active());
        assertEquals(7, windows.outcome().sessionId());
        prepare(2, 1);
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.start().kind());
        windows.refresh();
        assertEquals(0, windows.outcome().sessionId(), "新炉次等待回执时不能回显上一炉结果");
        now = 6000;
        windows.refresh();
        windows.refresh();
        assertEquals(0, windows.outcome().sessionId(), "请求超时也不能把旧结算当成本炉结果");
        reachable = false;
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.advance().kind());
    }

    @Test void billetFailureWithoutSessionStillReachesTheRequestingWindow() {
        windows.open(pos, bounds);
        prepare(1, 0);
        windows.start();
        ForgeOutcomeStore.replace(new ForgeOutcomeStore.Snapshot(9, "iron_sword_v0", "waste",
            null, 0, null, "", 0, false));
        windows.refresh();
        assertFalse(windows.pending());
        assertEquals(9, windows.outcome().sessionId(), "没有建炉次的制坯失败也必须在窗口显示");
        manager.reset();
        windows.open(pos, bounds);
        assertEquals(0, windows.outcome().sessionId(), "重开空工位不能冒充刚收到的结算");
    }

    private static void applyOutcome(long id) {
        String json = "{\"v\":1,\"type\":\"forge_outcome\",\"session_id\":" + id + ",\"bucket\":\"perfect\"}";
        new ForgeOutcomeHandler().handle(ServerDataEnvelope.parse(json, json.length()).envelope());
    }

    @Test void closeReturnsPreparationButSessionAndDisconnectNeverRefund() {
        var state = windows.open(pos, bounds);
        windows.material(1L, false);
        windows.close(state);
        assertInstanceOf(ForgeIntent.Material.class, sent.get(1));
        var refund = (ForgeIntent.Material) sent.get(1);
        assertTrue(refund.returning(), "尚未收到投料回执也必须排队返还");
        assertNull(refund.instanceId(), "关闭返还全部，不使用过期库存版本逐个取回");
        sent.clear();
        prepare(3, 0);
        windows.open(pos, bounds);
        manager.reset();
        assertTrue(sent.isEmpty(), "连接清理不发送返还请求");
        ForgeSessionStore.replace(session(7, "billet", "{}"));
        state = windows.open(pos, bounds);
        windows.close(state);
        assertTrue(sent.isEmpty(), "已开炉材料不因关闭窗口退回");
    }

    private void prepare(int first, int second) {
        var materials = new ArrayList<InventoryItem>();
        if (first > 0) materials.add(material(1, first));
        if (second > 0) materials.add(material(2, second));
        InventoryStateStore.replace(InventoryModel.builder().materialPreparation("iron_sword_v0", pos, materials).build());
        windows.refresh();
    }

    private static InventoryItem material(long id, int count) {
        return InventoryItem.createFull(id, "mineral_fan_tie", "凡铁", 1, 1, 0.2, "common", "", count, 1, 0)
            .withMineralId("fan_tie");
    }

    private static ForgeSessionStore.Snapshot session(long id, String step, String json) {
        return new ForgeSessionStore.Snapshot(id, "iron_sword_v0", "铁剑", true, step, 0, 1, json);
    }
}
