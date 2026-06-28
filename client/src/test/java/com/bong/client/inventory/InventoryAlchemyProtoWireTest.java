package com.bong.client.inventory;

import bong.Envelope;
import com.bong.client.inventory.model.InventoryItem;
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
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * fix/alchemy-proto-drift —— 丹药 / 残卷 / 丹心 tooltip「真机不显」的端到端回归。
 *
 * <p><b>根因</b>（与 #780 ContainerSnapshot.owner_instance_id 同一类）：serde
 * {@code InventoryItemViewV1.alchemy} 在 proto wire 上无对应字段 →
 * {@code inventory_item_view_to_proto} 不搬 → 生产走 protobuf（agent_bridge cfg(not(test))）
 * 时 alchemy 被静默丢弃 → client {@link InventorySnapshotHandler#readAlchemyLines} 拿 null →
 * {@code alchemyLines} 为空 → tooltip 无丹药行。单测吃 JSON 路径永远抓不到。
 *
 * <p><b>本测试走真机生产链</b>：用生成的 proto builder 真构字节（含
 * {@code setAlchemy(AlchemyItemDataProto)}）→ {@link ProtoServerDataBridge#bridge}（生产解码路径）
 * → {@link ServerDataEnvelope#parse} → {@link InventorySnapshotHandler#handle} →
 * {@link InventoryStateStore#snapshot}，断言 placed item 的 {@code alchemyLines} 含期望 tooltip 行。
 *
 * <p>修复前 proto 无 {@code AlchemyItemDataProto} 类 / {@code InventoryItemView.setAlchemy}
 * → 本测试编译不过 → RED，恰好锁死本次修复。
 */
public class InventoryAlchemyProtoWireTest {

    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    /** 一个最小但完整（不会被 handler noOp）的 InventorySnapshot proto builder，含一个 3x3 容器。 */
    private static Envelope.InventorySnapshot.Builder baseSnapshot() {
        Envelope.InventorySnapshot.Builder snap = Envelope.InventorySnapshot.newBuilder()
                .setRevision(3)
                .addContainers(Envelope.ContainerSnapshot.newBuilder()
                        .setId("body_pocket")
                        .setName("贴身口袋")
                        .setRows(3)
                        .setCols(3)
                        .setQuickAccess(true))
                .setEquipped(Envelope.EquippedInventorySnapshot.newBuilder())
                .setBoneCoins(0)
                .setWeight(Envelope.InventoryWeight.newBuilder().setCurrent(0.0).setMax(100.0))
                .setRealm("Induce")
                .setQiCurrent(0.0)
                .setQiMax(100.0)
                .setBodyLevel(0.0);
        for (int i = 0; i < InventoryModel.HOTBAR_SIZE; i++) {
            snap.addHotbar(Envelope.HotbarSlot.newBuilder());
        }
        return snap;
    }

    /** 1x1 物品视图骨架（除 alchemy 外的必填字段）。 */
    private static Envelope.InventoryItemView.Builder baseItemView(long instanceId, String itemId, String name) {
        return Envelope.InventoryItemView.newBuilder()
                .setInstanceId(instanceId)
                .setItemId(itemId)
                .setDisplayName(name)
                .setGridWidth(1)
                .setGridHeight(1)
                .setWeight(0.1)
                .setRarity("uncommon")
                .setStackCount(1)
                .setSpiritQuality(1.0)
                .setDurability(1.0);
    }

    /** 把一个 placed item 塞进 body_pocket(0,0)，过真机生产链解析，返回该 item 的 alchemyLines。 */
    private static List<String> alchemyLinesThroughWire(Envelope.InventoryItemView itemView) {
        Envelope.InventorySnapshot snapshot = baseSnapshot()
                .addPlacedItems(Envelope.PlacedInventoryItem.newBuilder()
                        .setContainerId("body_pocket")
                        .setRow(0)
                        .setCol(0)
                        .setItem(itemView))
                .build();

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
                "桥输出的 legacyJson 应能被解析，实际 error=" + parsed.errorMessage());

        ServerDataDispatch dispatch = new InventorySnapshotHandler().handle(parsed.envelope());
        assertTrue(dispatch.handled(),
                "inventory_snapshot 应被 handler 接受（非 noOp），实际 log=" + dispatch.logMessage()
                + "；若 noOp 说明 alchemy 解析失败（readAlchemyLines→null→整条 item 被丢）或缺必填字段");

        InventoryModel model = InventoryStateStore.snapshot();
        InventoryItem item = model.gridItems().stream()
                .map(InventoryModel.GridEntry::item)
                .filter(it -> it.instanceId() == itemView.getInstanceId())
                .findFirst()
                .orElse(null);
        assertNotNull(item,
                "解析后 model 应含该 placed item（instance=" + itemView.getInstanceId()
                + "）；若缺失说明 parseInventoryItem 返 null——alchemy proto 丢字段 / kind 漂移会触发此路径");
        return item.alchemyLines();
    }

    // ── pill：丹药 tooltip 经 proto wire 存活 ──

    @Test
    void pillAlchemyLinesSurviveRealProtoWire() {
        Envelope.InventoryItemView item = baseItemView(1, "qing_xin_dan", "清心丹")
                .setAlchemy(Envelope.AlchemyItemDataProto.newBuilder()
                        .setKind("pill")
                        .setRecipeId("qing_xin_dan")
                        .setQualityTier(2)
                        .setEffectMultiplier(0.9)
                        .setConsecrated(true)
                        .setSideEffect(Envelope.AlchemyItemSideEffect.newBuilder()
                                .setTag("qi_drain_mild")))
                .build();

        List<String> lines = alchemyLinesThroughWire(item);
        assertEquals(3, lines.size(),
                "pill 应产出 3 行（丹药品阶 + 开光 + 副作用），实际 " + lines);
        assertEquals("丹药 2阶 · 效力 90%", lines.get(0),
                "首行须含 quality_tier=2 / effect_multiplier=0.9，二者必须经 proto wire 存活，实际 " + lines.get(0));
        assertEquals("开光 · 持续翻倍", lines.get(1),
                "consecrated=true 须产出开光行，实际 " + lines.get(1));
        assertEquals("副作用 qi_drain_mild", lines.get(2),
                "side_effect.tag 须经 proto wire 存活并渲染，实际 " + lines.get(2));
    }

    // ── recipe_fragment：丹方残卷 tooltip 经 proto wire 存活 ──

    @Test
    void recipeFragmentAlchemyLinesSurviveRealProtoWire() {
        Envelope.InventoryItemView item = baseItemView(2, "alchemy_recipe_fragment", "丹方残卷")
                .setAlchemy(Envelope.AlchemyItemDataProto.newBuilder()
                        .setKind("recipe_fragment")
                        .setFragment(Envelope.AlchemyItemFragment.newBuilder()
                                .setRecipeId("huang_long_dan")
                                .addKnownStages(0)
                                .addKnownStages(2)
                                .setMaxQualityTier(3)))
                .build();

        List<String> lines = alchemyLinesThroughWire(item);
        assertEquals(2, lines.size(),
                "recipe_fragment 应产出 2 行（残卷名 + 已知段/上限），实际 " + lines);
        assertEquals("丹方残卷 huang_long_dan", lines.get(0),
                "fragment.recipe_id 须经 proto wire 存活，实际 " + lines.get(0));
        assertEquals("已知 2段 · 上限3阶", lines.get(1),
                "known_stages(size=2) / max_quality_tier=3 须经 proto wire 存活，实际 " + lines.get(1));
    }

    // ── recipe_hint：丹心线索 tooltip 经 proto wire 存活 ──

    @Test
    void recipeHintAlchemyLinesSurviveRealProtoWire() {
        Envelope.InventoryItemView item = baseItemView(3, "alchemy_recipe_hint", "丹心线索")
                .setAlchemy(Envelope.AlchemyItemDataProto.newBuilder()
                        .setKind("recipe_hint")
                        .setHint(Envelope.AlchemyItemHint.newBuilder()
                                .setSourcePill("qing_xin_dan")
                                .setAccuracy(0.75)
                                .addIngredients("ci_she_hao")
                                .addIngredients("ning_mai_cao")))
                .build();

        List<String> lines = alchemyLinesThroughWire(item);
        assertEquals(2, lines.size(),
                "recipe_hint 应产出 2 行（准度 + 药痕），实际 " + lines);
        assertEquals("丹心线索 · 准度 75%", lines.get(0),
                "hint.accuracy=0.75 须经 proto wire 存活并渲染，实际 " + lines.get(0));
        assertEquals("药痕 ci_she_hao/ning_mai_cao", lines.get(1),
                "hint.ingredients 须经 proto wire 存活并 join，实际 " + lines.get(1));
    }

    // ── 非丹药物品：alchemy 不写 wire → 无 tooltip 行（守恒反向 case）──

    @Test
    void nonAlchemyItemHasNoAlchemyLinesThroughWire() {
        Envelope.InventoryItemView item = baseItemView(4, "qing_feng_sword", "青锋剑").build();
        List<String> lines = alchemyLinesThroughWire(item);
        assertTrue(lines.isEmpty(),
                "非丹药物品 alchemy 缺省（proto 不写 wire）→ readAlchemyLines element==null → List.of()，"
                + "tooltip 不应有丹药行，实际 " + lines);
    }
}
