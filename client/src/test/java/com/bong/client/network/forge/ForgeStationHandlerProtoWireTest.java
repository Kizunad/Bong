package com.bong.client.network.forge;

import bong.Envelope;
import com.bong.client.forge.state.ForgeStationStore;
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
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-forge-session-entry-wiring-v1 §4.1#3（新发现连带缺口）—— {@code forge_station} 快照
 * 的 {@code pos} 经真实 protobuf wire 到达 client 的回归测试。
 *
 * <p>proto {@code ForgeStation}（{@code proto/bong/envelope.proto}）把 pos 拆成 flat
 * {@code int32 station_pos_x/y/z} 三字段（与 {@code AlchemyFurnace} 的
 * {@code optional int32 pos_x/y/z} 不同——这里非 optional，因为 Rust
 * {@code WeaponForgeStationDataV1.pos} 是 {@code (i32,i32,i32)} 而非
 * {@code Option<(i32,i32,i32)>}，砧的坐标恒已知）。{@link ProtoServerDataBridge} 对
 * {@code FORGE_STATION} 走通用路径，不做 flat→array 重塑；{@link ForgeStationHandler#handle}
 * 必须直接读 {@code station_pos_x}/{@code station_pos_y}/{@code station_pos_z} 三个顶层整数字段。</p>
 *
 * <p>本测试走真机生产链：proto builder 真构字节 → {@link ProtoServerDataBridge#bridge}
 * （生产解码路径）→ {@link ServerDataEnvelope#parse} → {@link ForgeStationHandler#handle} →
 * {@link ForgeStationStore#snapshot()}，断言经 proto wire 后 pos 与服务端下发坐标一致。</p>
 */
class ForgeStationHandlerProtoWireTest {

    @BeforeEach
    void setUp() {
        ForgeStationStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        ForgeStationStore.resetForTests();
    }

    private static Envelope.ForgeStation.Builder baseStation() {
        return Envelope.ForgeStation.newBuilder()
            .setStationId("forge_station_1")
            .setTier(2)
            .setIntegrity(0.85f)
            .setOwnerName("Kiz")
            .setHasSession(false);
    }

    private static ServerDataDispatch dispatchThroughWire(Envelope.ForgeStation station) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setForgeStation(station)
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

        return new ForgeStationHandler().handle(parsed.envelope());
    }

    // ── happy path：正负混合坐标经 proto wire 存活 ──

    @Test
    void stationPosSurvivesRealProtoWire() {
        Envelope.ForgeStation station = baseStation()
            .setStationPosX(12)
            .setStationPosY(66)
            .setStationPosZ(-8)
            .build();

        ServerDataDispatch dispatch = dispatchThroughWire(station);
        assertTrue(dispatch.handled(),
            "forge_station 应被 handler 接受（非 noOp），实际 log=" + dispatch.logMessage());

        ForgeStationStore.Snapshot snap = ForgeStationStore.snapshot();
        assertEquals(new BlockPos(12, 66, -8), snap.pos(),
            "server 下发 station_pos_x=12/y=66/z=-8（flat int32），经 proto wire 后 store.pos() "
            + "必须精确还原，实际 " + snap.pos());
        assertEquals(2, snap.tier(), "tier 应随快照一起经 proto wire 存活，实际 " + snap.tier());
        assertEquals("Kiz", snap.ownerName(), "owner_name 应随快照一起经 proto wire 存活，实际 " + snap.ownerName());
        assertEquals("forge_station_1", snap.stationId());
    }

    // ── 边界：显式坐标 (0,0,0) 不能被误判为「未设置」──

    @Test
    void stationZeroCoordinateBoundarySurvivesThroughWire() {
        Envelope.ForgeStation station = baseStation()
            .setStationPosX(0)
            .setStationPosY(0)
            .setStationPosZ(0)
            .build();

        ServerDataDispatch dispatch = dispatchThroughWire(station);
        assertTrue(dispatch.handled(), "forge_station 应被 handler 接受，实际 log=" + dispatch.logMessage());

        assertEquals(new BlockPos(0, 0, 0), ForgeStationStore.snapshot().pos(),
            "显式设置的 (0,0,0) 是合法真值，不应被降级为 null，实际 " + ForgeStationStore.snapshot().pos());
    }

    // ── 边界：大数值/双负坐标不发生截断或符号错乱 ──

    @Test
    void stationLargeMagnitudeCoordinatesSurviveThroughWire() {
        Envelope.ForgeStation station = baseStation()
            .setStationPosX(-1_000_000)
            .setStationPosY(320)
            .setStationPosZ(999_999)
            .build();

        ServerDataDispatch dispatch = dispatchThroughWire(station);
        assertTrue(dispatch.handled(), "forge_station 应被 handler 接受，实际 log=" + dispatch.logMessage());

        assertEquals(new BlockPos(-1_000_000, 320, 999_999), ForgeStationStore.snapshot().pos(),
            "int32 全量级坐标（大负数 + 大正数）经 proto wire 不应发生截断/符号错乱，实际 "
            + ForgeStationStore.snapshot().pos());
    }

    // ── 状态转换：has_session 前后两种状态都应经 wire 正确到达 ──

    @Test
    void hasSessionTrueSurvivesThroughWire() {
        Envelope.ForgeStation station = baseStation()
            .setStationPosX(5)
            .setStationPosY(70)
            .setStationPosZ(5)
            .setHasSession(true)
            .build();

        ServerDataDispatch dispatch = dispatchThroughWire(station);
        assertTrue(dispatch.handled());
        assertTrue(ForgeStationStore.snapshot().hasSession(),
            "has_session=true 应随快照一起经 proto wire 存活，实际 " + ForgeStationStore.snapshot().hasSession());
    }
}
