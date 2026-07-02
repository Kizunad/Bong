package com.bong.client.network.alchemy;

import bong.Envelope;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * fix/proto-worldpos-flat-cluster —— 生产 {@code --release} 下丹炉位置经 protobuf 传输丢失的端到端回归。
 *
 * <p><b>根因</b>：proto {@code AlchemyFurnace}（{@code proto/bong/envelope.proto}）把
 * {@code Option<(i32,i32,i32)>} 拆成 flat {@code optional int32 pos_x/pos_y/pos_z} 三字段
 * （非嵌套数组）。{@link ProtoServerDataBridge#bridge} 对 {@code ALCHEMY_FURNACE} 走通用路径，
 * 不做 flat→array 重塑，桥出的 legacy JSON 顶层直接是 {@code pos_x}/{@code pos_y}/{@code pos_z}
 * 三个整数字段，绝不会出现 {@code "pos":[x,y,z]} 数组。修复前的
 * {@link AlchemyFurnaceHandler#handle} 用 {@code getAsJsonArray("pos")} 读取，生产链下永远
 * 拿不到 → {@code pos} 恒为 {@code null} → 丹炉位置在 --release 客户端上彻底丢失（快照其余字段
 * 如 tier/integrity 仍会正常应用，只是位置丢了，UI 无法定位丹炉方块）。
 *
 * <p><b>本测试走真机生产链</b>：用生成的 proto builder 真构字节（{@code setPosX/Y/Z}）→
 * {@link ProtoServerDataBridge#bridge}（生产解码路径）→ {@link ServerDataEnvelope#parse} →
 * {@link AlchemyFurnaceHandler#handle} → {@link AlchemyFurnaceStore#snapshot()}，断言经 proto
 * wire 后丹炉 {@code pos} 与服务端下发坐标一致。
 *
 * <p>修复前：本测试的 happy-path / 零坐标 / 大数值坐标三个 case 全部因
 * {@code getAsJsonArray("pos")} 读不到旧字段名而拿 {@code null} → RED。
 */
class AlchemyFurnaceHandlerProtoWireTest {

    @BeforeEach
    void setUp() {
        AlchemyFurnaceStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        AlchemyFurnaceStore.resetForTests();
    }

    /** 除 pos 外的必填字段骨架（tier/integrity/owner/has_session 各自可覆盖）。 */
    private static Envelope.AlchemyFurnace.Builder baseFurnace() {
        return Envelope.AlchemyFurnace.newBuilder()
                .setTier(2)
                .setIntegrity(0.92)
                .setIntegrityMax(1.0)
                .setOwnerName("Kiz")
                .setHasSession(true);
    }

    /** 把 AlchemyFurnace proto 过真机生产链（proto bytes → bridge → parse → handle），返回 dispatch 结果。 */
    private static ServerDataDispatch dispatchThroughWire(Envelope.AlchemyFurnace furnace) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAlchemyFurnace(furnace)
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

        return new AlchemyFurnaceHandler().handle(parsed.envelope());
    }

    // ── happy path：正负混合坐标经 proto wire 存活 ──

    @Test
    void furnacePosSurvivesRealProtoWire() {
        Envelope.AlchemyFurnace furnace = baseFurnace()
                .setPosX(10)
                .setPosY(64)
                .setPosZ(-3)
                .build();

        ServerDataDispatch dispatch = dispatchThroughWire(furnace);
        assertTrue(dispatch.handled(),
                "alchemy_furnace 应被 handler 接受（非 noOp），实际 log=" + dispatch.logMessage());

        AlchemyFurnaceStore.Snapshot snap = AlchemyFurnaceStore.snapshot();
        assertEquals(new BlockPos(10, 64, -3), snap.pos(),
                "server 下发 pos_x=10/pos_y=64/pos_z=-3（flat int32），经 proto wire 后 store.pos() 必须精确还原；"
                + "修复前 handler 读旧数组字段名 \"pos\" 在生产链下恒为空 → pos 恒 null，实际 " + snap.pos());
        assertEquals(2, snap.tier(), "tier 应随快照一起经 proto wire 存活，实际 " + snap.tier());
        assertEquals("Kiz", snap.ownerName(), "owner_name 应随快照一起经 proto wire 存活，实际 " + snap.ownerName());
        assertTrue(snap.hasSession(), "has_session=true 应随快照一起经 proto wire 存活，实际 " + snap.hasSession());
    }

    // ── 边界：显式坐标 (0,0,0) 不能被误判为「未设置」──

    @Test
    void furnaceZeroCoordinateBoundarySurvivesThroughWire() {
        // pos_x/y/z 是 proto3 optional（显式 presence），(0,0,0) 是合法真值而非「缺省未设」。
        // 若 handler 误用「非零才算存在」之类的判断，会把合法原点坐标错误降级为 null。
        Envelope.AlchemyFurnace furnace = baseFurnace()
                .setPosX(0)
                .setPosY(0)
                .setPosZ(0)
                .build();

        ServerDataDispatch dispatch = dispatchThroughWire(furnace);
        assertTrue(dispatch.handled(), "alchemy_furnace 应被 handler 接受，实际 log=" + dispatch.logMessage());

        BlockPos pos = AlchemyFurnaceStore.snapshot().pos();
        assertEquals(new BlockPos(0, 0, 0), pos,
                "显式设置的 (0,0,0) 是合法真值（proto3 optional 显式 presence），不应被降级为 null，实际 " + pos);
    }

    // ── 边界：大数值/双负坐标不发生截断或符号错乱 ──

    @Test
    void furnaceLargeMagnitudeCoordinatesSurviveThroughWire() {
        Envelope.AlchemyFurnace furnace = baseFurnace()
                .setPosX(-1_000_000)
                .setPosY(320)
                .setPosZ(999_999)
                .build();

        ServerDataDispatch dispatch = dispatchThroughWire(furnace);
        assertTrue(dispatch.handled(), "alchemy_furnace 应被 handler 接受，实际 log=" + dispatch.logMessage());

        BlockPos pos = AlchemyFurnaceStore.snapshot().pos();
        assertEquals(new BlockPos(-1_000_000, 320, 999_999), pos,
                "int32 全量级坐标（大负数 + 大正数）经 proto wire 不应发生截断/符号错乱，实际 " + pos);
    }

    // ── 状态转换：server 未下发 pos（丹炉尚未放置）→ pos 优雅降级为 null，快照其余字段仍应用 ──

    @Test
    void furnaceWithoutPosDegradesToNullButAppliesRestOfSnapshotThroughWire() {
        // 不调用 setPosX/Y/Z：proto3 optional 三字段均处于 unset 状态，
        // JsonFormat 对显式 presence 字段不会强填默认值，桥出的 legacyJson 里
        // pos_x/pos_y/pos_z 三键全部缺失。此时降级语义须是「只丢坐标」而非「整条丢弃」。
        Envelope.AlchemyFurnace furnace = baseFurnace()
                .setTier(3)
                .setHasSession(false)
                .build();

        ServerDataDispatch dispatch = dispatchThroughWire(furnace);
        assertTrue(dispatch.handled(),
                "缺 pos 不应让整条 alchemy_furnace 快照被拒（noOp），应仅 pos 字段降级，实际 log="
                + dispatch.logMessage());

        AlchemyFurnaceStore.Snapshot snap = AlchemyFurnaceStore.snapshot();
        assertNull(snap.pos(),
                "server 未下发 pos_x/pos_y/pos_z（丹炉未放置）→ pos 应优雅降级为 null，实际 " + snap.pos());
        assertEquals(3, snap.tier(),
                "pos 缺失不应连累快照其余字段——tier 仍须正常应用，实际 " + snap.tier());
        assertFalse(snap.hasSession(),
                "pos 缺失不应连累快照其余字段——has_session 仍须正常应用，实际 " + snap.hasSession());
    }
}
