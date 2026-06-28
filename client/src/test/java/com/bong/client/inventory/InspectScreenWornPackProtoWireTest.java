package com.bong.client.inventory;

import bong.Envelope;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.InventorySnapshotHandler;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * fix/worn-pack-tab-missing —— 穿戴背包件「不显 tab」真机 bug 的端到端回归。
 *
 * <p><b>根因</b>：生产 wire 走 protobuf（agent_bridge cfg(not(test))），而 #779 只把
 * {@code owner_instance_id} / {@code quick_access} 加进 serde 结构体 + 走 JSON 的测试路径，
 * 漏了 proto 定义 + proto 转换。于是这俩字段过 protobuf 线时被静默丢弃 → client
 * {@code ContainerDef.ownerInstanceId == null} → {@link InspectScreen#computeContainerDefs}
 * 短路 skip pack 容器 → 穿戴态也不出右侧 tab。
 *
 * <p><b>为何之前的 headless 测试没抓到</b>：{@code InspectScreenContainerRefreshTest} /
 * {@code InventorySnapshotQuickAccessParseTest} 用 builder 直构 model 或喂裸整数 JSON，
 * <b>绕过了 proto wire</b>。本测试用<b>生成的 proto builder 真构字节</b>，喂
 * {@link ProtoServerDataBridge#bridge}（生产解码路径，过 printAndNormalize 的
 * uint64-as-string → number 归一化）→ {@link ServerDataEnvelope#parse} →
 * {@link InventorySnapshotHandler#handle} → {@link InventoryStateStore#snapshot} →
 * {@link InspectScreen#computeContainerDefs}，走真机同一条线。
 *
 * <p>#779 当时 {@code ContainerSnapshot} proto builder 连 {@code setOwnerInstanceId} 方法都没有
 * → 本测试编译不过 → RED，恰好锁死本次修复。
 */
public class InspectScreenWornPackProtoWireTest {

    private static final long PACK_OWNER_INSTANCE = 12L;

    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    /** 一个最小但完整（不会被 handler noOp）的 InventorySnapshot proto builder。 */
    private static Envelope.InventorySnapshot.Builder baseSnapshot() {
        Envelope.InventorySnapshot.Builder snap = Envelope.InventorySnapshot.newBuilder()
                .setRevision(7)
                // body_pocket：恒 tab、owner=None（不写 wire）、quick_access=true。
                .addContainers(Envelope.ContainerSnapshot.newBuilder()
                        .setId("body_pocket")
                        .setName("贴身口袋")
                        .setRows(2)
                        .setCols(3)
                        .setQuickAccess(true))
                .setEquipped(Envelope.EquippedInventorySnapshot.newBuilder())
                .setBoneCoins(0)
                .setWeight(Envelope.InventoryWeight.newBuilder().setCurrent(0.0).setMax(100.0))
                .setRealm("Induce")
                .setQiCurrent(0.0)
                .setQiMax(100.0)
                .setBodyLevel(0.0);
        // hotbar 必须恰好 HOTBAR_SIZE(9) 槽，否则 parseHotbar → null → noOp（与真机一致）。
        for (int i = 0; i < InventoryModel.HOTBAR_SIZE; i++) {
            snap.addHotbar(Envelope.HotbarSlot.newBuilder());
        }
        return snap;
    }

    /** 穿戴背包件视图：chest_worn 含 instance=12 的 InventoryItemView。 */
    private static Envelope.InventoryItemView.Builder packItemView() {
        return Envelope.InventoryItemView.newBuilder()
                .setInstanceId(PACK_OWNER_INSTANCE)
                .setItemId("worn_grass_pouch")
                .setDisplayName("破草包")
                .setGridWidth(2)
                .setGridHeight(2)
                .setWeight(0.3)
                .setRarity("common")
                .setStackCount(1)
                .setSpiritQuality(1.0)
                .setDurability(1.0);
    }

    /** 走真机生产链：proto bytes → bridge → parse → handle → store snapshot。 */
    private static InventoryModel bridgeAndApply(Envelope.InventorySnapshot snapshot) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(snapshot)
                .build();

        ProtoServerDataBridge.BridgeResult bridged =
                ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
                "proto→legacyJson 桥接应成功，实际 error=" + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
                legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(),
                "桥输出的 legacyJson 应能被 ServerDataEnvelope 解析，实际 error=" + parsed.errorMessage());

        ServerDataDispatch dispatch = new InventorySnapshotHandler().handle(parsed.envelope());
        assertTrue(dispatch.handled(),
                "inventory_snapshot 应被 handler 接受（非 noOp），实际 log=" + dispatch.logMessage()
                + "；若 noOp 说明 snapshot 缺必填字段或 proto 桥丢了内容");
        return InventoryStateStore.snapshot();
    }

    // ── 穿戴态：owner_instance_id 经 proto wire 存活 → pack tab 出现 ──

    @Test
    void wornPackProducesTabThroughRealProtoWire() {
        Envelope.InventorySnapshot snapshot = baseSnapshot()
                .addContainers(Envelope.ContainerSnapshot.newBuilder()
                        .setId("pack_" + PACK_OWNER_INSTANCE)
                        .setName("破草包")
                        .setRows(3)
                        .setCols(3)
                        // 这是修复点：proto 必须带 owner_instance_id / quick_access，否则过线丢失。
                        .setOwnerInstanceId(PACK_OWNER_INSTANCE)
                        .setQuickAccess(true))
                // 背包件穿戴在 chest worn 层（instance=12，与 owner_instance_id 对齐）。
                .setEquipped(Envelope.EquippedInventorySnapshot.newBuilder()
                        .addChestWorn(packItemView()))
                .build();

        InventoryModel model = bridgeAndApply(snapshot);

        // owner_instance_id 必须经 proto wire + bridge uint64→number 归一化后非 null。
        InventoryModel.ContainerDef packDef = model.containers().stream()
                .filter(d -> ("pack_" + PACK_OWNER_INSTANCE).equals(d.id()))
                .findFirst()
                .orElse(null);
        assertNotNull(packDef, "解析后 model 应含 pack_12 容器 def");
        assertEquals(Long.valueOf(PACK_OWNER_INSTANCE), packDef.ownerInstanceId(),
                "pack_12 的 ownerInstanceId 必须 == 12（proto wire 携带 owner_instance_id，"
                + "经 normalizeNumericStrings 把 \"12\" 回转成 number 后由 readOptionalLong 读出）；"
                + "若为 null 说明 proto 层又吞了字段——正是本 bug 根因");
        assertTrue(packDef.quickAccess(),
                "pack_12 的 quickAccess 必须经 proto wire 存活为 true");

        // 端到端目标：computeContainerDefs 必须含 pack_12 tab（穿戴态显常驻 tab）。
        var defs = InspectScreen.computeContainerDefs(model);
        assertTrue(defs.stream().anyMatch(d -> ("pack_" + PACK_OWNER_INSTANCE).equals(d.id())),
                "穿戴背包件（owner=12 在 chest worn 层）经真实 proto wire → 桥 → 解析后，"
                + "computeContainerDefs 必须含 pack_12 tab；缺失 = 穿戴态不显 tab 的真机 bug 复发");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id(),
                "body_pocket 仍恒 index 0（决议 #18 不变）");
    }

    // ── 反向（旧 server / 当货物态）：proto 不带 owner_instance_id → owner==null → 不出 tab ──

    @Test
    void packWithoutOwnerInWireProducesNoTab() {
        Envelope.InventorySnapshot snapshot = baseSnapshot()
                .addContainers(Envelope.ContainerSnapshot.newBuilder()
                        .setId("pack_" + PACK_OWNER_INSTANCE)
                        .setName("破草包")
                        .setRows(3)
                        .setCols(3))
                // 不设 owner_instance_id（旧 server 缺字段），也不穿戴该件。
                .build();

        InventoryModel model = bridgeAndApply(snapshot);

        InventoryModel.ContainerDef packDef = model.containers().stream()
                .filter(d -> ("pack_" + PACK_OWNER_INSTANCE).equals(d.id()))
                .findFirst()
                .orElse(null);
        assertNotNull(packDef, "解析后 model 仍应含 pack_12 容器 def（只是 owner 为 null）");
        assertEquals(null, packDef.ownerInstanceId(),
                "proto 未携带 owner_instance_id 时（proto3 optional 省键），ownerInstanceId 应为 null");

        var defs = InspectScreen.computeContainerDefs(model);
        assertFalse(defs.stream().anyMatch(d -> ("pack_" + PACK_OWNER_INSTANCE).equals(d.id())),
                "owner==null（旧 server 缺字段 / 当货物态）→ pack 不进 tab（保持 #778 不退化为显示）");
    }
}
