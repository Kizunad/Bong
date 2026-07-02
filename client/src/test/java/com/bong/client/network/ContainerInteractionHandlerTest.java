package com.bong.client.network;

import bong.Envelope;
import com.bong.client.hud.SearchHudState;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.tsy.TsyContainerStateStore;
import com.bong.client.tsy.TsyContainerView;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * fix/proto-worldpos-flat-cluster — {@code container_state} 的 {@code world_pos} 坐标在
 * 生产 proto wire 上「读丢」回归。
 *
 * <p><b>根因</b>：{@code ContainerStateProto}（proto/bong/envelope.proto）把 Rust
 * {@code [f64;3]} 拆成三个 flat 字段 {@code world_pos_x/world_pos_y/world_pos_z}。
 * {@link ProtoServerDataBridge} 用 {@code JsonFormat.preservingProtoFieldNames()} 把 proto
 * 转成 legacy JSON 时**不会**把这三个 flat 字段重塑回 JSON 数组 —— 生产（{@code --release}）
 * 下走的正是这条 proto 路径。旧版 {@link ContainerInteractionHandler#handle} 用
 * {@code readDoubleTriple(payload, "world_pos")} 读**数组**，在 proto-bridged JSON 里永远
 * 拿 null → 整条 {@code container_state} 更新被 noOp 丢弃 → {@link TsyContainerStateStore}
 * 里的坐标永远不会被写入/刷新。
 *
 * <p>{@link #containerStateAppliesFlatWorldPosThroughRealProtoWire()} 走真机生产链
 * （proto builder 真构字节 → {@link ProtoServerDataBridge#bridge} → {@link ServerDataEnvelope#parse}
 * → {@link ContainerInteractionHandler#handle}），锁死修复：flat world_pos_x/y/z 必须能正确
 * 落进 store。修复前该测试因 {@code dispatch.handled()} 为 false 而 RED。
 */
public class ContainerInteractionHandlerTest {
    @AfterEach
    void tearDown() {
        TsyContainerStateStore.resetForTests();
        SearchHudStateStore.resetForTests();
    }

    @Test
    void routerRegistersContainerInteractionPayloads() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        assertTrue(router.registeredTypes().contains("container_state"));
        assertTrue(router.registeredTypes().contains("search_started"));
        assertTrue(router.registeredTypes().contains("search_progress"));
        assertTrue(router.registeredTypes().contains("search_completed"));
        assertTrue(router.registeredTypes().contains("search_aborted"));
    }

    @Test
    void containerStateFeedsStoreAndSearchHud() {
        // Legacy JSON now carries world_pos flat (world_pos_x/_y/_z), matching the shape
        // ContainerStateProto actually produces on the real proto wire (see
        // containerStateAppliesFlatWorldPosThroughRealProtoWire below) — the handler no
        // longer accepts a "world_pos":[x,y,z] array.
        route("""
            {"type":"container_state","v":1,"entity_id":42,"kind":"storage_pouch","family_id":"tsy","world_pos_x":1.0,"world_pos_y":2.0,"world_pos_z":3.0,"depleted":false}
            """);

        assertEquals(42L, TsyContainerStateStore.get(42L).entityId());
        assertEquals("储物袋残骸", TsyContainerStateStore.get(42L).kindLabelZh());
        assertEquals(1.0, TsyContainerStateStore.get(42L).x());
        assertEquals(2.0, TsyContainerStateStore.get(42L).y());
        assertEquals(3.0, TsyContainerStateStore.get(42L).z());

        route("""
            {"type":"search_started","v":1,"player_id":"offline:Kiz","container_entity_id":42,"required_ticks":200,"at_tick":10}
            """);
        assertEquals(SearchHudState.Phase.SEARCHING, SearchHudStateStore.snapshot().phase());
        assertEquals("储物袋残骸", SearchHudStateStore.snapshot().containerKindZh());

        route("""
            {"type":"search_progress","v":1,"player_id":"offline:Kiz","container_entity_id":42,"elapsed_ticks":20,"required_ticks":200}
            """);
        assertEquals(20, SearchHudStateStore.snapshot().elapsedTicks());

        route("""
            {"type":"search_aborted","v":1,"player_id":"offline:Kiz","container_entity_id":42,"reason":"cancelled","at_tick":30}
            """);
        assertEquals(SearchHudState.Phase.ABORTED_FLASH, SearchHudStateStore.snapshot().phase());
        assertEquals(SearchHudState.AbortReason.CANCELLED, SearchHudStateStore.snapshot().abortReason());
    }

    // ── container_state: flat world_pos survives the real production proto wire ──

    @Test
    void containerStateAppliesFlatWorldPosThroughRealProtoWire() {
        Envelope.ContainerStateProto containerState = Envelope.ContainerStateProto.newBuilder()
                .setEntityId(77L)
                .setKind(Envelope.ContainerKind.CONTAINER_KIND_STONE_CASKET)
                .setFamilyId("tsy")
                .setWorldPosX(11.5)
                .setWorldPosY(64.0)
                .setWorldPosZ(-200.25)
                .setDepleted(false)
                .build();

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setContainerState(containerState)
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

        ServerDataDispatch dispatch = new ContainerInteractionHandler().handle(parsed.envelope());
        assertTrue(dispatch.handled(),
                "container_state 应被 handler 接受（非 noOp），实际 log=" + dispatch.logMessage()
                + "；若 noOp 说明 world_pos 在 proto wire 上以 flat world_pos_x/_y/_z 字段落地，"
                + "而 handler 仍按 JSON 数组读取 → pos==null → 整条 container_state 被丢弃"
                + "（本次要锁死的回归：修复前必 RED）。");

        TsyContainerView view = TsyContainerStateStore.get(77L);
        assertNotNull(view, "proto wire 解析成功后 store 里应有 entity_id=77 的容器视图");
        assertEquals(11.5, view.x(),
                "world_pos_x=11.5 须经 proto wire（flat 字段）正确落进 store.x，"
                + "实际 " + view.x() + "；若为 0.0 说明坐标又被当数组读丢");
        assertEquals(64.0, view.y(),
                "world_pos_y=64.0 须经 proto wire（flat 字段）正确落进 store.y，实际 " + view.y());
        assertEquals(-200.25, view.z(),
                "world_pos_z=-200.25 须经 proto wire（flat 字段）正确落进 store.z，实际 " + view.z());
        assertEquals("tsy", view.familyId(),
                "family_id 应随 world_pos 一起正常存活（非本次回归目标，但顺带验证整条 container_state 未被破坏）");
    }

    // ── container_state: ContainerKind / KeyKind 枚举前缀在 proto wire 上被剥除 ──
    //
    // fix/container-state-enum-prefix：proto3 canonical JSON 把枚举打成 CONTAINER_KIND_* /
    // KEY_KIND_*，而 TsyContainerView.kindLabelZh() 与 serde 侧（container-state sample）期望
    // snake_case。修复前 CONTAINER_STATE 走 generic bridge 路径不剥前缀 → kind 全落「容器」默认标签、
    // locked 也带 KEY_KIND_ 前缀。ProtoServerDataBridge 新增 bridgeContainerState 剥两者前缀。

    @Test
    void containerKindAndLockedEnumPrefixStrippedThroughProtoWire() {
        TsyContainerView view = viewThroughProtoWire(Envelope.ContainerStateProto.newBuilder()
                .setEntityId(88L)
                .setKind(Envelope.ContainerKind.CONTAINER_KIND_STONE_CASKET)
                .setFamilyId("tsy")
                .setWorldPosX(1.0).setWorldPosY(2.0).setWorldPosZ(3.0)
                .setLocked(Envelope.KeyKind.KEY_KIND_STONE_CASKET_KEY)
                .setDepleted(false));

        assertNotNull(view, "proto wire 解析成功后 store 里应有 entity_id=88 的容器视图");
        assertEquals("stone_casket", view.kind(),
                "ContainerKind.CONTAINER_KIND_STONE_CASKET 须被 bridge 剥前缀+小写成 serde snake_case "
                + "\"stone_casket\"，实际 \"" + view.kind() + "\"；若仍是 CONTAINER_KIND_* 说明枚举前缀未剥（修复前 RED）");
        assertEquals("石匣", view.kindLabelZh(),
                "剥前缀后 kindLabelZh() 应命中 \"stone_casket\"→\"石匣\"，实际 \"" + view.kindLabelZh()
                + "\"；修复前 kind 带 CONTAINER_KIND_ 前缀 → 落 default「容器」");
        assertEquals("stone_casket_key", view.locked(),
                "KeyKind.KEY_KIND_STONE_CASKET_KEY 须被剥前缀+小写成 \"stone_casket_key\"，实际 \"" + view.locked() + "\"");
    }

    @Test
    void containerKindStoragePouchStrippedAndUnlockedOptionalAbsentThroughProtoWire() {
        // 第二个 kind 变体 + optional locked 不 set（未上锁）→ locked 缺省为 null（合法降级），
        // 不应出现带前缀的字符串。同时 world_pos 全 0 边界不应被误判「缺失」。
        TsyContainerView view = viewThroughProtoWire(Envelope.ContainerStateProto.newBuilder()
                .setEntityId(89L)
                .setKind(Envelope.ContainerKind.CONTAINER_KIND_STORAGE_POUCH)
                .setFamilyId("tsy")
                .setWorldPosX(0.0).setWorldPosY(0.0).setWorldPosZ(0.0)
                .setDepleted(false));

        assertNotNull(view, "store 里应有 entity_id=89 的容器视图");
        assertEquals("storage_pouch", view.kind(),
                "CONTAINER_KIND_STORAGE_POUCH 应剥成 \"storage_pouch\"，实际 \"" + view.kind() + "\"");
        assertEquals("储物袋残骸", view.kindLabelZh(),
                "\"storage_pouch\"→\"储物袋残骸\"，实际 \"" + view.kindLabelZh() + "\"");
        assertNull(view.locked(),
                "未 set 的 optional locked 应缺省为 null（未上锁），不应出现带前缀的字符串，实际 \"" + view.locked() + "\"");
    }

    /** 过真机生产链解析一个 ContainerStateProto，返回落进 store 的 view。 */
    private static TsyContainerView viewThroughProtoWire(Envelope.ContainerStateProto.Builder builder) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setContainerState(builder)
                .build();
        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(), "proto→legacyJson 桥接应成功，实际 error=" + bridged.errorMessage());
        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
                legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(), "legacyJson 应能被解析，实际 error=" + parsed.errorMessage());
        ServerDataDispatch dispatch = new ContainerInteractionHandler().handle(parsed.envelope());
        assertTrue(dispatch.handled(),
                "container_state 应被 handler 接受，实际 log=" + dispatch.logMessage());
        return TsyContainerStateStore.get(builder.getEntityId());
    }

    private static void route(String json) {
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json.strip(), json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(result.dispatch().handled(), result.dispatch().logMessage());
    }
}
