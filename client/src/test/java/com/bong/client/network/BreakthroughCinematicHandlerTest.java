package com.bong.client.network;

import bong.Envelope;
import com.bong.client.cultivation.BreakthroughRenderState;
import com.bong.client.cultivation.BreakthroughRenderStateStore;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F6：BreakthroughCinematicHandler 饱和单测。
 *
 * <p>覆盖：
 * <ul>
 *   <li>五个阶段（prelude/charge/catalyze/apex/aftermath）各自触发的 vfxEventIds 粒子 + 差异化音效</li>
 *   <li>aftermath 按 result/interrupted 分叉（success / failure / interrupted）</li>
 *   <li>粒子 origin 精确匹配 payload world_pos（而非本地玩家坐标）</li>
 *   <li>空 vfxEventIds / 空 audioRecipeId 不崩、不误触发</li>
 *   <li>malformed payload（noOp）不触发任何粒子/音效副作用</li>
 *   <li>动画调用在无 MinecraftClient 的单测环境下安全短路，不抛异常</li>
 * </ul>
 *
 * <p>注入风格仿 {@link ShieldBrokenHandlerTest}。
 */
class BreakthroughCinematicHandlerTest {

    private final List<AudioEventPayload.PlaySoundRecipe> playedRecipes = new ArrayList<>();
    private final AudioPlaybackBridge stubAudio = new AudioPlaybackBridge() {
        @Override
        public boolean play(AudioEventPayload.PlaySoundRecipe payload) {
            playedRecipes.add(payload);
            return true;
        }

        @Override
        public boolean stop(AudioEventPayload.StopSoundRecipe payload) {
            return false;
        }
    };

    private final List<VfxEventPayload.SpawnParticle> spawnedParticles = new ArrayList<>();
    private final VfxParticleBridge stubParticle = payload -> {
        spawnedParticles.add(payload);
        return true;
    };

    @BeforeEach
    void setUp() {
        BreakthroughCinematicHandler.setAudioBridgeForTests(stubAudio);
        BreakthroughCinematicHandler.setParticleBridgeForTests(stubParticle);
        BreakthroughRenderStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        BreakthroughCinematicHandler.resetAudioBridgeForTests();
        BreakthroughCinematicHandler.resetParticleBridgeForTests();
        BreakthroughRenderStateStore.resetForTests();
    }

    // ---- 既有契约（router-level）：保留原有两条回归用例 -------------------

    @Test
    void handlesBreakthroughCinematicServerDataWithVisualAndToast() {
        ServerDataRouter.RouteResult result = route(apexJson());

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("breakthrough_cinematic", result.envelope().type());
        assertTrue(result.dispatch().visualEffectState().isPresent());
        assertTrue(result.dispatch().alertToast().isPresent());
        assertTrue(result.logMessage().contains("bong:breakthrough_pillar"));
    }

    @Test
    void rejectsMalformedBreakthroughCinematicPayloadSafely() {
        String json = """
            {
              "v": 1,
              "type": "breakthrough_cinematic",
              "phase": "apex"
            }
            """;

        ServerDataRouter.RouteResult result = route(json);

        assertFalse(result.isParseError());
        assertTrue(result.isNoOp());
    }

    // ---- 粒子：按阶段触发,origin 用 payload world_pos ----------------------

    @Test
    void preludePhase_spawnsCultivationAbsorbParticle() {
        handle(phaseJson("prelude", "pending", false));
        assertFalse(spawnedParticles.isEmpty(),
            "prelude 阶段应触发粒子，但 spawnedParticles 为空");
        assertTrue(
            spawnedParticles.stream().anyMatch(p -> p.eventId().equals(new Identifier("bong", "cultivation_absorb"))),
            "prelude 阶段应触发 bong:cultivation_absorb 粒子；实际=" + eventIds()
        );
    }

    @Test
    void chargePhase_spawnsTwoDistinctParticles() {
        handle(phaseJson("charge", "pending", false));
        List<Identifier> ids = eventIds();
        assertEquals(2, ids.size(),
            "charge 阶段 vfxEventIds 应有 2 条（cultivation_absorb + meridian_open），实际=" + ids);
        assertTrue(ids.contains(new Identifier("bong", "cultivation_absorb")));
        assertTrue(ids.contains(new Identifier("bong", "meridian_open")));
    }

    @Test
    void catalyzePhase_spawnsBreakthroughPillarParticle() {
        handle(phaseJson("catalyze", "pending", false));
        assertTrue(
            eventIds().contains(new Identifier("bong", "breakthrough_pillar")),
            "catalyze 阶段应触发 bong:breakthrough_pillar 粒子；实际=" + eventIds()
        );
    }

    @Test
    void apexPhase_spawnsBreakthroughPillarParticle() {
        handle(phaseJson("apex", "pending", false));
        assertTrue(
            eventIds().contains(new Identifier("bong", "breakthrough_pillar")),
            "apex 阶段应触发 bong:breakthrough_pillar 粒子；实际=" + eventIds()
        );
    }

    @Test
    void aftermathSuccess_spawnsBreakthroughPillarParticle() {
        handle(phaseJson("aftermath", "success", false));
        assertTrue(
            eventIds().contains(new Identifier("bong", "breakthrough_pillar")),
            "aftermath 成功应触发 bong:breakthrough_pillar 粒子（余韵光柱）；实际=" + eventIds()
        );
    }

    @Test
    void aftermathFailure_spawnsBreakthroughFailParticle_notPillar() {
        handle(phaseJson("aftermath", "failure", false));
        List<Identifier> ids = eventIds();
        assertTrue(
            ids.contains(new Identifier("bong", "breakthrough_fail")),
            "aftermath 失败应触发 bong:breakthrough_fail 粒子；实际=" + ids
        );
        assertFalse(
            ids.contains(new Identifier("bong", "breakthrough_pillar")),
            "aftermath 失败不应触发成功用的 breakthrough_pillar 粒子（差异化要求）；实际=" + ids
        );
    }

    @Test
    void aftermathInterrupted_spawnsBreakthroughFailParticle() {
        handle(phaseJson("aftermath", "pending", true));
        assertTrue(
            eventIds().contains(new Identifier("bong", "breakthrough_fail")),
            "被打断的 aftermath 应触发 bong:breakthrough_fail 粒子；实际=" + eventIds()
        );
    }

    @Test
    void particleOrigin_matchesPayloadWorldPos_notLocalPlayerPos() {
        handle(phaseJson("prelude", "pending", false, new double[]{123.5, 70.0, -400.25}));
        assertFalse(spawnedParticles.isEmpty());
        double[] origin = spawnedParticles.get(0).origin();
        assertEquals(123.5, origin[0], 1e-9, "origin.x 应等于 payload world_pos[0]");
        assertEquals(70.0, origin[1], 1e-9, "origin.y 应等于 payload world_pos[1]");
        assertEquals(-400.25, origin[2], 1e-9, "origin.z 应等于 payload world_pos[2]");
    }

    // ---- 音效：按 audioRecipeId 差异化 --------------------------------------

    @Test
    void preludePhase_playsHeartbeatSlowRecipe() {
        handle(phaseJson("prelude", "pending", false));
        assertFalse(playedRecipes.isEmpty(), "prelude 阶段应触发音效播放");
        assertEquals("breakthrough_heartbeat_slow", playedRecipes.get(0).recipeId());
    }

    @Test
    void chargePhase_playsHeartbeatFastRecipe_notSlow() {
        handle(phaseJson("charge", "pending", false));
        assertFalse(playedRecipes.isEmpty());
        String recipeId = playedRecipes.get(0).recipeId();
        assertEquals("breakthrough_heartbeat_fast", recipeId);
        assertFalse("breakthrough_heartbeat_slow".equals(recipeId),
            "charge 音效应快于 prelude（差异化要求），不能复用同一 recipeId");
    }

    @Test
    void apexPhase_playsBellRecipe() {
        handle(phaseJson("apex", "pending", false));
        assertEquals("breakthrough_bell", playedRecipes.get(0).recipeId());
    }

    @Test
    void aftermathFailure_playsFailureRecipe_notAfterglow() {
        handle(phaseJson("aftermath", "failure", false));
        String recipeId = playedRecipes.get(0).recipeId();
        assertEquals("breakthrough_failure", recipeId);
        assertFalse("breakthrough_afterglow".equals(recipeId));
    }

    @Test
    void aftermathInterrupted_playsInterruptedRecipe_notFailure() {
        handle(phaseJson("aftermath", "pending", true));
        String recipeId = playedRecipes.get(0).recipeId();
        assertEquals("breakthrough_interrupted", recipeId,
            "被打断应播 breakthrough_interrupted，与普通失败区分");
        assertFalse("breakthrough_failure".equals(recipeId));
    }

    @Test
    void aftermathSuccess_playsAfterglowRecipe() {
        handle(phaseJson("aftermath", "success", false));
        assertEquals("breakthrough_afterglow", playedRecipes.get(0).recipeId());
    }

    @Test
    void recipeLayersAreNonEmpty_forEveryPhase() {
        for (String phase : List.of("prelude", "charge", "catalyze", "apex")) {
            playedRecipes.clear();
            handle(phaseJson(phase, "pending", false));
            assertFalse(playedRecipes.get(0).recipe().layers().isEmpty(),
                "阶段 " + phase + " 的 AudioRecipe 不应为空层");
        }
    }

    // ---- 空/异常输入不崩 ----------------------------------------------------

    @Test
    void malformedPayload_doesNotTriggerParticleOrAudio() {
        String json = """
            {
              "v": 1,
              "type": "breakthrough_cinematic",
              "phase": "apex"
            }
            """;
        handle(json);
        assertTrue(spawnedParticles.isEmpty(),
            "malformed payload（noOp）不应触发任何粒子；实际=" + eventIds());
        assertTrue(playedRecipes.isEmpty(),
            "malformed payload（noOp）不应触发任何音效；实际=" + playedRecipes);
    }

    @Test
    void handle_doesNotThrow_whenMinecraftClientUnavailable() {
        // 单测环境没有 MinecraftClient 实例；triggerAnimation 应静默跳过而非抛异常。
        assertDoesNotThrow(() -> handle(phaseJson("apex", "pending", false)),
            "无 MinecraftClient 环境下 handle() 不应抛异常（动画调用需安全短路）");
    }

    // ---- F7：渲染状态写入 BreakthroughRenderStateStore ------------------------

    @Test
    void handle_writesPayloadToRenderStateStore() {
        handle(phaseJson("apex", "pending", false, new double[]{5.0, 66.0, 7.0}));
        BreakthroughRenderState state = BreakthroughRenderStateStore.snapshot();
        assertTrue(state != null, "handle() 后 BreakthroughRenderStateStore 应有快照，实际为 null");
        assertEquals(5.0, state.payload().worldX(), 1e-9);
        assertEquals(66.0, state.payload().worldY(), 1e-9);
        assertEquals(7.0, state.payload().worldZ(), 1e-9);
    }

    @Test
    void handle_renderStateExpiryIsInFuture_relativeToHandleTime() {
        long before = System.currentTimeMillis();
        handle(phaseJson("apex", "pending", false));
        long after = System.currentTimeMillis();
        BreakthroughRenderState state = BreakthroughRenderStateStore.snapshot();
        assertTrue(state.expiresAtMillis() >= before,
            "expiresAtMillis 应不早于 handle() 调用前的时间戳");
        assertFalse(state.isExpired(after),
            "刚写入的状态在 handle() 后立刻检查不应视为已过期");
    }

    @Test
    void handle_secondPayload_overwritesRenderState() {
        handle(phaseJson("prelude", "pending", false, new double[]{1.0, 2.0, 3.0}));
        handle(phaseJson("apex", "pending", false, new double[]{9.0, 8.0, 7.0}));
        BreakthroughRenderState state = BreakthroughRenderStateStore.snapshot();
        assertEquals(9.0, state.payload().worldX(), 1e-9,
            "第二次 handle() 应覆盖 store 中的第一次快照");
    }

    @Test
    void malformedPayload_doesNotWriteRenderStateStore() {
        handle("{\"v\":1,\"type\":\"breakthrough_cinematic\",\"phase\":\"apex\"}");
        assertTrue(BreakthroughRenderStateStore.snapshot() == null,
            "malformed payload（noOp）不应写入 BreakthroughRenderStateStore");
    }

    @Test
    void toastAndFlash_stillPresent_alongsideNewSideEffects() {
        // 防回归：新增副作用不能破坏既有 toast/flash 契约（F6 修复前唯一存在的部分）。
        ServerDataDispatch dispatch = handle(phaseJson("apex", "pending", false));
        assertTrue(dispatch.alertToast().isPresent(), "toast 应仍存在");
        assertTrue(dispatch.visualEffectState().isPresent(), "视觉特效应仍存在");
        assertFalse(spawnedParticles.isEmpty(), "粒子应有触发");
        assertFalse(playedRecipes.isEmpty(), "音效应有触发");
    }

    // ── fix/proto-worldpos-flat-cluster：breakthrough_cinematic 经真机 proto 生产链回归 ──
    //
    // 根因：proto BreakthroughCinematic（proto/bong/envelope.proto 2332-2352 行）把 Rust
    // [f64;3] world_pos 拆成三个 flat 字段 world_pos_x/world_pos_y/world_pos_z。
    // ProtoServerDataBridge 对这个 payload 走通用 extractInner 路径（无专属 bridgeXxx 重塑函数），
    // 用 JsonFormat.preservingProtoFieldNames() 直接转 legacy JSON，不做 flat→array 重塑——
    // 生产（--release）下走的正是这条 proto 路径。旧版 BreakthroughCinematicPayload.parse 用
    // readVec3(payload, "world_pos") 读**数组**形状，在 proto-bridged JSON 里永远拿 null →
    // 整条 payload parse() 返回 null → handler noOp → 突破演出（toast/视觉特效/粒子/音效/
    // 动作/远景 billboard）整段被静默丢弃。上面用手写 legacy JSON 的用例测不出这个坑，因为它们
    // 不经过 ProtoServerDataBridge，本节改走真机生产链。
    //
    // 本测试不手写 legacy JSON，而是用生成的 proto builder 真构字节 → 走
    // ProtoServerDataBridge.bridge（生产解码路径）→ ServerDataEnvelope.parse →
    // BreakthroughCinematicHandler.handle，断言 dispatch 非 noOp 且 BreakthroughRenderStateStore
    // 落地的坐标与写入的 world_pos_x/y/z 一致。修复前：readVec3 读数组 → worldPos==null →
    // parse() 返回 null → dispatch.handled() 为 false，本测试 RED。

    @Test
    void breakthroughCinematicSurvivesRealProtoWireFlatCoords() {
        Envelope.BreakthroughCinematic cinematic = Envelope.BreakthroughCinematic.newBuilder()
            .setActorId("offline:Kiz")
            .setPhase("apex")
            .setPhaseTick(10)
            .setPhaseDurationTicks(80)
            .setRealmFrom("Condense")
            .setRealmTo("Solidify")
            .setResult("success")
            .setInterrupted(false)
            .setWorldPosX(123.5)
            .setWorldPosY(70.0)
            .setWorldPosZ(-400.25)
            .setVisibleRadiusBlocks(1024.0)
            .setGlobal(false)
            .setDistantBillboard(true)
            .setParticleDensity(2.2f)
            .setIntensity(0.78f)
            .setSeasonOverlay("adaptive")
            .setStyle("golden_core")
            .setAtTick(2400)
            .build();

        ServerDataDispatch dispatch = handleThroughRealProtoWire(cinematic);

        assertTrue(dispatch.handled(),
            "breakthrough_cinematic 应被 handler 接受（非 noOp），因为 world_pos_x/_y/_z 均已在 proto 里写入；"
            + "实际 log=" + dispatch.logMessage()
            + "——若为 noOp 说明 BreakthroughCinematicPayload.parse 仍按数组形状读取 \"world_pos\"，"
            + "未识别 proto 生产链的 flat 三字段");

        BreakthroughRenderState state = BreakthroughRenderStateStore.snapshot();
        assertTrue(state != null,
            "handle() 后 BreakthroughRenderStateStore 应有快照，实际为 null"
            + "（world_pos 解析失败会让 handler 在写 store 之前就 return noOp）");
        assertEquals(123.5, state.payload().worldX(), 1e-9,
            "world_pos_x=123.5 须经 proto wire 存活到 payload.worldX()，实际 " + state.payload().worldX());
        assertEquals(70.0, state.payload().worldY(), 1e-9,
            "world_pos_y=70.0 须经 proto wire 存活到 payload.worldY()，实际 " + state.payload().worldY());
        assertEquals(-400.25, state.payload().worldZ(), 1e-9,
            "world_pos_z=-400.25 须经 proto wire 存活到 payload.worldZ()，实际 " + state.payload().worldZ());
    }

    @Test
    void breakthroughCinematicZeroWorldPosNotMisreadAsMissing_throughRealProtoWire() {
        // proto3 scalar 字段没有独立的"未设置"位——不 set 就是 wire 上的 default 0.0，且 0.0
        // 仍是一个合法坐标（出生点附近完全可能突破）。这条锁的是 readDouble 用 isNumber() 判断
        // 而非"truthy/非零"判断，防止把合法的 0.0 坐标误判为缺失导致整条演出被 noOp 丢弃。
        Envelope.BreakthroughCinematic cinematic = Envelope.BreakthroughCinematic.newBuilder()
            .setActorId("offline:Kiz")
            .setPhase("prelude")
            .setPhaseTick(0)
            .setPhaseDurationTicks(40)
            .setRealmFrom("Awaken")
            .setRealmTo("Induce")
            .setResult("pending")
            .setInterrupted(false)
            .setWorldPosX(0.0)
            .setWorldPosY(0.0)
            .setWorldPosZ(0.0)
            .setVisibleRadiusBlocks(64.0)
            .setGlobal(false)
            .setDistantBillboard(false)
            .setParticleDensity(1.0f)
            .setIntensity(0.4f)
            .setSeasonOverlay("adaptive")
            .setStyle("fresh_spiral")
            .setAtTick(0)
            .build();

        ServerDataDispatch dispatch = handleThroughRealProtoWire(cinematic);

        assertTrue(dispatch.handled(),
            "world_pos 分量恰好为 0.0（合法坐标）不应被误判为缺失而 noOp，实际 log=" + dispatch.logMessage());
        BreakthroughRenderState state = BreakthroughRenderStateStore.snapshot();
        assertTrue(state != null, "0.0 坐标是合法输入，应正常落地 render state，实际为 null");
        assertEquals(0.0, state.payload().worldX(), 1e-9);
        assertEquals(0.0, state.payload().worldY(), 1e-9);
        assertEquals(0.0, state.payload().worldZ(), 1e-9);
    }

    @Test
    void particleOrigin_matchesRealProtoWorldPos_notLocalPlayerPos() {
        // 二重验证：不仅 render state store 落地正确坐标，triggerParticles 用 payload.worldX/Y/Z
        // 构造的粒子 origin 也必须经真机 proto wire 存活（镜像上面手写 JSON 版本的
        // particleOrigin_matchesPayloadWorldPos_notLocalPlayerPos）。
        Envelope.BreakthroughCinematic cinematic = Envelope.BreakthroughCinematic.newBuilder()
            .setActorId("offline:Kiz")
            .setPhase("prelude")
            .setPhaseTick(0)
            .setPhaseDurationTicks(40)
            .setRealmFrom("Awaken")
            .setRealmTo("Induce")
            .setResult("pending")
            .setInterrupted(false)
            .setWorldPosX(55.5)
            .setWorldPosY(80.0)
            .setWorldPosZ(-12.25)
            .setVisibleRadiusBlocks(64.0)
            .setGlobal(false)
            .setDistantBillboard(false)
            .setParticleDensity(1.0f)
            .setIntensity(0.4f)
            .setSeasonOverlay("adaptive")
            .setStyle("fresh_spiral")
            .setAtTick(0)
            .build();

        handleThroughRealProtoWire(cinematic);

        assertFalse(spawnedParticles.isEmpty(),
            "prelude 阶段应触发至少一个粒子事件（若为空说明 payload 在 particle 派发前已被 noOp 丢弃）");
        double[] origin = spawnedParticles.get(0).origin();
        assertEquals(55.5, origin[0], 1e-9, "粒子 origin.x 应取自真机 proto wire 的 world_pos_x");
        assertEquals(80.0, origin[1], 1e-9, "粒子 origin.y 应取自真机 proto wire 的 world_pos_y");
        assertEquals(-12.25, origin[2], 1e-9, "粒子 origin.z 应取自真机 proto wire 的 world_pos_z");
    }

    /** proto builder → 真机字节 → ProtoServerDataBridge → legacy JSON → BreakthroughCinematicHandler.handle()。 */
    private static ServerDataDispatch handleThroughRealProtoWire(Envelope.BreakthroughCinematic cinematic) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setBreakthroughCinematic(cinematic)
            .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
            "proto→legacyJson 桥接应成功（breakthrough_cinematic 走通用 extractInner 路径），实际 error="
            + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(),
            "桥输出的 legacyJson 应能被 ServerDataEnvelope 解析，实际 error=" + parsed.errorMessage()
            + "，legacyJson=" + legacyJson);

        return new BreakthroughCinematicHandler().handle(parsed.envelope());
    }

    // ---- 辅助 ---------------------------------------------------------------

    private List<Identifier> eventIds() {
        return spawnedParticles.stream().map(VfxEventPayload.SpawnParticle::eventId).toList();
    }

    private ServerDataDispatch handle(String json) {
        return new BreakthroughCinematicHandler().handle(parseEnvelope(json));
    }

    private ServerDataRouter.RouteResult route(String json) {
        return ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json,
            json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), "parse failed: " + parseResult.errorMessage());
        return parseResult.envelope();
    }

    private static String apexJson() {
        return phaseJson("apex", "success", false, new double[]{12.0, 64.0, -8.0});
    }

    private static String phaseJson(String phase, String result, boolean interrupted) {
        return phaseJson(phase, result, interrupted, new double[]{12.0, 64.0, -8.0});
    }

    private static String phaseJson(String phase, String result, boolean interrupted, double[] worldPos) {
        return "{"
            + "\"v\":1,"
            + "\"type\":\"breakthrough_cinematic\","
            + "\"actor_id\":\"offline:Kiz\","
            + "\"phase\":\"" + phase + "\","
            + "\"phase_tick\":0,"
            + "\"phase_duration_ticks\":80,"
            + "\"realm_from\":\"Condense\","
            + "\"realm_to\":\"Solidify\","
            + "\"result\":\"" + result + "\","
            + "\"interrupted\":" + interrupted + ","
            // proto BreakthroughCinematic 把 [f64;3] world_pos 拆平成 world_pos_x/_y/_z 三个
            // flat 字段（见 BreakthroughCinematicPayload.parse 的 fix-worldpos-cluster 修复），
            // ProtoServerDataBridge 不做 flat→array 重塑，故 legacy JSON 也须用 flat 形状。
            + "\"world_pos_x\":" + worldPos[0] + ","
            + "\"world_pos_y\":" + worldPos[1] + ","
            + "\"world_pos_z\":" + worldPos[2] + ","
            + "\"visible_radius_blocks\":1024.0,"
            + "\"global\":false,"
            + "\"distant_billboard\":true,"
            + "\"particle_density\":2.2,"
            + "\"intensity\":0.78,"
            + "\"season_overlay\":\"adaptive\","
            + "\"style\":\"golden_core\","
            + "\"at_tick\":2400"
            + "}";
    }
}
