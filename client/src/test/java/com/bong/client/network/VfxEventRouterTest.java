package com.bong.client.network;

import com.bong.client.animation.AnimWiringManifestTest;
import com.bong.client.animation.BongAnimationPlayer;
import com.bong.client.animation.ClientAnimationBridge;
import com.bong.client.animation.ProductionAnimationResources;
import com.google.gson.JsonObject;
import dev.kosmx.playerAnim.api.layered.AnimationStack;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.OptionalInt;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class VfxEventRouterTest {
    private static final UUID FIXTURE_UUID = UUID.fromString("550e8400-e29b-41d4-a716-446655440000");

    @Test
    void dispatchesPlayAnimToBridge() throws IOException {
        RecordingBridge bridge = new RecordingBridge(true);
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-play-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isHandled(), "result should be handled: " + result.logMessage());
        assertEquals(1, bridge.playCalls.size());
        RecordingBridge.PlayCall call = bridge.playCalls.get(0);
        assertEquals(FIXTURE_UUID, call.target);
        assertEquals(new Identifier("bong", "sword_swing_horiz"), call.animId);
        assertEquals(1000, call.priority);
        assertEquals(OptionalInt.of(3), call.fadeInTicks);
    }

    @Test
    void dispatchesPlayAnimInlineToBridge() throws IOException {
        RecordingBridge bridge = new RecordingBridge(true);
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-play-anim-inline.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isHandled(), "result should be handled: " + result.logMessage());
        assertEquals(1, bridge.inlineCalls.size());
        RecordingBridge.InlineCall call = bridge.inlineCalls.get(0);
        assertEquals(FIXTURE_UUID, call.target);
        assertEquals(new Identifier("bong", "inline_test_pose"), call.animId);
        assertTrue(call.animJson.contains("inline_test_pose"));
        assertEquals(3000, call.priority);
        assertEquals(OptionalInt.of(3), call.fadeInTicks);
    }

    @Test
    void dispatchesStopAnimToBridge() throws IOException {
        RecordingBridge bridge = new RecordingBridge(true);
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-stop-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isHandled());
        assertEquals(1, bridge.stopCalls.size());
        RecordingBridge.StopCall call = bridge.stopCalls.get(0);
        assertEquals(new Identifier("bong", "meditate_sit"), call.animId);
        assertEquals(OptionalInt.of(5), call.fadeOutTicks);
    }

    @Test
    void parseErrorShortCircuitsBeforeBridge() throws IOException {
        RecordingBridge bridge = new RecordingBridge(true);
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("invalid-vfx-bad-uuid.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isParseError());
        assertEquals(0, bridge.playCalls.size());
        assertEquals(0, bridge.inlineCalls.size());
        assertEquals(0, bridge.stopCalls.size());
    }

    @Test
    void bridgeDeclineBecomesBridgeMiss() throws IOException {
        RecordingBridge bridge = new RecordingBridge(false);
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-play-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss());
        assertFalse(result.isHandled());
        assertEquals(1, bridge.playCalls.size(), "bridge is still invoked; just returns false");
    }

    @Test
    void dispatchesSpawnParticleToParticleBridge() throws IOException {
        RecordingBridge animBridge = new RecordingBridge(true);
        RecordingParticleBridge particleBridge = new RecordingParticleBridge(true);
        VfxEventRouter router = new VfxEventRouter(animBridge, particleBridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-spawn-particle.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isHandled(), "result should be handled: " + result.logMessage());
        assertEquals(0, animBridge.playCalls.size(), "animation bridge must not be touched");
        assertEquals(1, particleBridge.calls.size());
        VfxEventPayload.SpawnParticle dispatched = particleBridge.calls.get(0);
        assertEquals(new Identifier("bong", "sword_qi_slash"), dispatched.eventId());
    }

    @Test
    void spawnParticleFallsBackToBridgeMissWhenUnregistered() throws IOException {
        VfxEventRouter router = new VfxEventRouter(new RecordingBridge(true));
        // noop default bridge always declines; simulate unregistered event_id path
        String json = PayloadFixtureLoader.readText("valid-vfx-spawn-particle.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss());
        assertTrue(result.logMessage().contains("spawn_particle"), result.logMessage());
    }

    @Test
    void dispatchesPlayEntityAnimToEntityBridge() throws IOException {
        RecordingBridge animBridge = new RecordingBridge(true);
        RecordingEntityBridge entityBridge = new RecordingEntityBridge(true);
        VfxEventRouter router = new VfxEventRouter(animBridge, VfxParticleBridge.noop(), entityBridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-play-entity-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isHandled(), "result should be handled: " + result.logMessage());
        assertEquals(0, animBridge.playCalls.size(), "player animation bridge must not be touched");
        assertEquals(1, entityBridge.calls.size());
        RecordingEntityBridge.Call call = entityBridge.calls.get(0);
        assertEquals(42, call.entityId);
        assertEquals("animation.bong.heiwushi.dark_barrage", call.anim);
        assertEquals(15, call.durationTicks);
    }

    @Test
    void playEntityAnimDeclineBecomesBridgeMiss() throws IOException {
        RecordingEntityBridge entityBridge = new RecordingEntityBridge(false);
        VfxEventRouter router = new VfxEventRouter(new RecordingBridge(true), VfxParticleBridge.noop(), entityBridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-play-entity-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss());
        assertEquals(1, entityBridge.calls.size(), "bridge is still invoked; just returns false");
        assertTrue(result.logMessage().contains("play_entity_anim"), result.logMessage());
    }

    @Test
    void playEntityAnimFallsBackToBridgeMissWhenNoEntityBridge() throws IOException {
        // 默认（1-arg / 2-arg）router 用 noop entity bridge → 总是 false → bridgeMiss。
        VfxEventRouter router = new VfxEventRouter(new RecordingBridge(true));
        String json = PayloadFixtureLoader.readText("valid-vfx-play-entity-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss());
        assertTrue(result.logMessage().contains("play_entity_anim"), result.logMessage());
    }

    @Test
    void bridgeExceptionBecomesBridgeMissNotCrash() throws IOException {
        ThrowingBridge bridge = new ThrowingBridge();
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-play-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss());
        assertTrue(result.logMessage().contains("IllegalStateException"), result.logMessage());
    }

    @Test
    void entityBridgeExceptionBecomesBridgeMissNotCrash() throws IOException {
        // play_entity_anim 路径的 entity bridge 抛异常 → router try/catch 兜底转 bridgeMiss，
        // 而非让运行期失败撕裂整个 vfx_event 网络层。日志钉住异常类型供降级 warn。
        ThrowingEntityBridge entityBridge = new ThrowingEntityBridge();
        VfxEventRouter router =
            new VfxEventRouter(new RecordingBridge(true), VfxParticleBridge.noop(), entityBridge);
        String json = PayloadFixtureLoader.readText("valid-vfx-play-entity-anim.json");

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss(), "entity bridge 抛异常应转 bridgeMiss: " + result.logMessage());
        assertFalse(result.isHandled());
        assertTrue(result.logMessage().contains("IllegalStateException"), result.logMessage());
    }

    // ---- plan-skill-av-relink-v1 P3：P1 接线清单 → 路由契约（动画链客户端闭环） ----

    /**
     * 清单驱动的路由契约：server P1 每条新接线发射的 anim_id（共享清单
     * {@code bong/anim_wiring_manifest.json}，与 server {@code P1_WIRED_ANIM_IDS} 单向同步）
     * 构成 play_anim payload 后，经真实 {@link VfxEventRouter#route} 全部可达
     * {@link VfxEventAnimationBridge}，且 animId / priority / fade_in_ticks 三字段原样透传。
     * priority / fade 按条目错开取值，防"恰好等于默认值"的假透传。
     */
    @Test
    void dispatchesEveryManifestWiredPlayAnimToBridge() throws IOException {
        List<String> manifest = AnimWiringManifestTest.loadManifest();
        for (int i = 0; i < manifest.size(); i++) {
            String animId = manifest.get(i);
            int priority = 1000 + i;
            int fadeIn = 1 + (i % 5);
            RecordingBridge bridge = new RecordingBridge(true);
            VfxEventRouter router = new VfxEventRouter(bridge);
            String json = playAnimJson(animId, priority, fadeIn);

            VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

            assertTrue(result.isHandled(),
                "接线 anim `" + animId + "` 的 play_anim payload 应被路由处理，实际："
                    + result.logMessage());
            assertEquals(1, bridge.playCalls.size(),
                "接线 anim `" + animId + "` 应恰好触发一次 bridge.playAnim");
            RecordingBridge.PlayCall call = bridge.playCalls.get(0);
            assertEquals(FIXTURE_UUID, call.target, animId + " 目标玩家透传");
            assertEquals(Identifier.tryParse(animId), call.animId, animId + " anim id 透传");
            assertEquals(priority, call.priority, animId + " priority 透传");
            assertEquals(OptionalInt.of(fadeIn), call.fadeInTicks, animId + " fade_in_ticks 透传");
        }
    }

    /**
     * 真实生产 bridge 链：play_anim 经 route() 进 {@link ClientAnimationBridge}。
     * headless 单测环境无 MinecraftClient/world，目标玩家不可解析 → bridge 返回 false →
     * 路由降级 bridgeMiss——锁住"生产 bridge 在无运行时环境下安全降级、不撕裂网络层"。
     */
    @Test
    void playAnimThroughRealClientAnimationBridgeDegradesToBridgeMissHeadless() throws IOException {
        List<String> manifest = AnimWiringManifestTest.loadManifest();
        VfxEventRouter router = new VfxEventRouter(new ClientAnimationBridge());
        String json = playAnimJson(manifest.get(0), 1000, 3);

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss(),
            "headless 环境下真实 ClientAnimationBridge 应降级 bridgeMiss（目标玩家不可解析），"
                + "实际：" + result.logMessage());
        assertFalse(result.isHandled());
    }

    /**
     * P3 正向消费闭环（plan 明文验收项）：清单每条已知 anim id 的 play_anim payload
     * 经<b>真实</b> {@link VfxEventRouter} → <b>真实</b> {@link ClientAnimationBridge}
     * →（{@code AnimationLayerManager} → {@code BongAnimationPlayer}）成功消费——
     * 动画资产经生产 resource reload 入口装载（非手工灌注），bridge 仅注入目标解析
     * 接缝（headless 无 MinecraftClient，解析后的派发路径与生产完全同一条），断言
     * RouteResult 为 handled、非 bridgeMiss，且动画层真的装进了注入的
     * {@link AnimationStack}（activeAnimations 命中 + 按 priority 可从该 stack 摘除）。
     */
    @Test
    void everyManifestWiredAnimPlaysThroughRealClientAnimationBridgeNonMiss() throws IOException {
        ProductionAnimationResources.loadViaProductionReloadCallback();
        List<String> manifest = AnimWiringManifestTest.loadManifest();
        for (int i = 0; i < manifest.size(); i++) {
            String animIdRaw = manifest.get(i);
            Identifier animId = Identifier.tryParse(animIdRaw);
            int priority = 1000 + i;
            UUID target = UUID.randomUUID();
            AnimationStack stack = new AnimationStack();
            ClientAnimationBridge bridge = new ClientAnimationBridge(
                uuid -> target.equals(uuid)
                    ? new ClientAnimationBridge.AnimationTarget(stack, uuid)
                    : null);
            VfxEventRouter router = new VfxEventRouter(bridge);
            String json = playAnimJsonFor(target, animIdRaw, priority, 1 + (i % 5));

            VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

            assertTrue(result.isHandled(),
                "已知接线 anim `" + animIdRaw + "` 经真实 ClientAnimationBridge 应为 handled"
                    + "（生产 reload 已装载资产、目标可解析），实际：" + result.logMessage());
            assertFalse(result.isBridgeMiss(),
                "已知接线 anim `" + animIdRaw + "` 不得降级 bridgeMiss——"
                    + "真实 bridge 正向闭环断了（注册表查询或层安装失败）");
            assertTrue(BongAnimationPlayer.activeAnimations(target).contains(animId),
                "已知接线 anim `" + animIdRaw + "` 应出现在目标玩家的活跃动画集——"
                    + "AnimationLayerManager/BongAnimationPlayer 未真正安装动画层");
            assertTrue(stack.removeLayer(priority),
                "按 priority " + priority + " 应能从注入的 AnimationStack 摘到动画层——"
                    + "层没装进被注入的 stack，说明 bridge 播放走了别的目标（非注入解析结果）");
        }
    }

    /**
     * 未知 anim_id 失败分支走<b>真实</b> {@link ClientAnimationBridge}（注入目标解析
     * 接缝，被测对象仍是生产 bridge 本身）：注册表查不到的 anim id →
     * {@code AnimationLayerManager} 播放返回 false → 路由记 bridgeMiss，不抛异常不崩溃。
     */
    @Test
    void unknownAnimIdBecomesBridgeMissNotCrashThroughRealBridge() throws IOException {
        UUID target = UUID.randomUUID();
        AnimationStack stack = new AnimationStack();
        ClientAnimationBridge bridge = new ClientAnimationBridge(
            uuid -> target.equals(uuid)
                ? new ClientAnimationBridge.AnimationTarget(stack, uuid)
                : null);
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = playAnimJsonFor(target, "bong:__vfx_router_unknown_anim__", 1000, 3);

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isBridgeMiss(),
            "未知 anim id 应走注册表 miss → bridgeMiss 降级，实际：" + result.logMessage());
        assertFalse(result.isHandled(),
            "未知 anim id 绝不能被标记为已处理（否则缺资产静默上线）");
        assertTrue(BongAnimationPlayer.activeAnimations(target).isEmpty(),
            "未知 anim id 不得在目标玩家上装出任何动画层");
    }

    /** wire 契约限 anim_json ≤4096 字节——inline 闭环用与 fixture 同构的最小合法 v3 JSON。 */
    private static final String INLINE_PROBE_ANIM_JSON = """
        {"version":3,"name":"__inline_closure_probe__","emote":{"beginTick":0,\
        "endTick":4,"isLoop":false,"moves":[{"tick":0,"rightArm":{"pitch":-0.6},\
        "easing":"LINEAR"},{"tick":4,"rightArm":{"pitch":0.4},"easing":"INOUTSINE"}]}}""";

    /**
     * inline 播放经<b>真实</b> {@link ClientAnimationBridge}：合法 Emotecraft v3
     * 字节的 play_anim_inline 经 route() 在注入 stack 上安装 inline 动画层（handled、
     * 非 bridgeMiss、activeAnimations 命中、层按 priority 可摘除）——锁住 inline
     * 路径迁移到目标解析接缝后的 stack/UUID 派发正确性。
     */
    @Test
    void playAnimInlineThroughRealClientAnimationBridgeInstallsLayerOnInjectedStack()
        throws IOException {
        UUID target = UUID.randomUUID();
        AnimationStack stack = new AnimationStack();
        ClientAnimationBridge bridge = new ClientAnimationBridge(
            uuid -> target.equals(uuid)
                ? new ClientAnimationBridge.AnimationTarget(stack, uuid)
                : null);
        VfxEventRouter router = new VfxEventRouter(bridge);
        Identifier inlineId = new Identifier("bong", "__inline_closure_probe__");
        String json = playAnimInlineJsonFor(
            target, inlineId.toString(), INLINE_PROBE_ANIM_JSON, 3000, 2);

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertTrue(result.isHandled(),
            "真实资产字节的 inline 播放经真实 bridge 应为 handled，实际：" + result.logMessage());
        assertFalse(result.isBridgeMiss(), "inline 正向闭环不得降级 bridgeMiss");
        assertTrue(BongAnimationPlayer.activeAnimations(target).contains(inlineId),
            "inline 动画应出现在目标玩家活跃动画集——inline 注册或层安装断了");
        assertTrue(stack.removeLayer(3000),
            "inline 层应装进注入的 AnimationStack（按 priority 3000 可摘除）——"
                + "层落到了别的 stack，说明 bridge 派发目标错误");
    }

    /**
     * inline 失败分支经真实 bridge：坏 anim_json（生产反序列化拒绝）与目标不可
     * 解析都降级 bridgeMiss、不装层、不崩溃。
     */
    @Test
    void playAnimInlineFailureBranchesDegradeToBridgeMissThroughRealBridge() throws IOException {
        UUID target = UUID.randomUUID();
        AnimationStack stack = new AnimationStack();
        ClientAnimationBridge bridge = new ClientAnimationBridge(
            uuid -> target.equals(uuid)
                ? new ClientAnimationBridge.AnimationTarget(stack, uuid)
                : null);
        VfxEventRouter router = new VfxEventRouter(bridge);

        String badJson = playAnimInlineJsonFor(
            target, "bong:__inline_bad_json_probe__", "not-a-valid-animation", 3000, 2);
        VfxEventRouter.RouteResult badResult = router.route(badJson, jsonLen(badJson));
        assertTrue(badResult.isBridgeMiss(),
            "坏 anim_json 应被生产反序列化拒绝并降级 bridgeMiss，实际：" + badResult.logMessage());
        assertTrue(BongAnimationPlayer.activeAnimations(target).isEmpty(),
            "坏 anim_json 不得在目标玩家上装出动画层");

        UUID unresolvable = UUID.randomUUID();
        String missJson = playAnimInlineJsonFor(
            unresolvable, "bong:__inline_no_target_probe__", "{}", 3000, 2);
        VfxEventRouter.RouteResult missResult = router.route(missJson, jsonLen(missJson));
        assertTrue(missResult.isBridgeMiss(),
            "目标不可解析的 inline 播放应降级 bridgeMiss，实际：" + missResult.logMessage());
    }

    /**
     * stop_anim 经<b>真实</b> {@link ClientAnimationBridge}：先经真实 bridge 播一条
     * 清单动画，再以 fade_out_ticks=0 停止——handled、活跃动画集移除、层已立即从
     * 注入 stack 摘除；未在播的 id / 目标不可解析降级 bridgeMiss。
     */
    @Test
    void stopAnimThroughRealClientAnimationBridgeRemovesInstalledLayer() throws IOException {
        ProductionAnimationResources.loadViaProductionReloadCallback();
        String animIdRaw = AnimWiringManifestTest.loadManifest().get(0);
        Identifier animId = Identifier.tryParse(animIdRaw);
        UUID target = UUID.randomUUID();
        AnimationStack stack = new AnimationStack();
        ClientAnimationBridge bridge = new ClientAnimationBridge(
            uuid -> target.equals(uuid)
                ? new ClientAnimationBridge.AnimationTarget(stack, uuid)
                : null);
        VfxEventRouter router = new VfxEventRouter(bridge);
        String playJson = playAnimJsonFor(target, animIdRaw, 1000, 3);
        assertTrue(router.route(playJson, jsonLen(playJson)).isHandled(),
            "前置：清单动画应先经真实 bridge 播放成功");
        assertTrue(BongAnimationPlayer.activeAnimations(target).contains(animId),
            "前置：播放后动画应在活跃集内");

        String stopJson = stopAnimJsonFor(target, animIdRaw, 0);
        VfxEventRouter.RouteResult result = router.route(stopJson, jsonLen(stopJson));

        assertTrue(result.isHandled(),
            "在播动画的 stop_anim 经真实 bridge 应为 handled，实际：" + result.logMessage());
        assertFalse(BongAnimationPlayer.activeAnimations(target).contains(animId),
            "stop 后动画不得留在目标玩家活跃动画集");
        assertFalse(stack.removeLayer(1000),
            "fade_out_ticks=0 应已立即把层从注入 stack 摘除——仍能按 priority 摘到"
                + "说明 stop 没作用到注入的 stack");
    }

    /** stop_anim 失败分支经真实 bridge：未在播的 id 与目标不可解析都降级 bridgeMiss。 */
    @Test
    void stopAnimFailureBranchesDegradeToBridgeMissThroughRealBridge() throws IOException {
        UUID target = UUID.randomUUID();
        AnimationStack stack = new AnimationStack();
        ClientAnimationBridge bridge = new ClientAnimationBridge(
            uuid -> target.equals(uuid)
                ? new ClientAnimationBridge.AnimationTarget(stack, uuid)
                : null);
        VfxEventRouter router = new VfxEventRouter(bridge);

        String notPlaying = stopAnimJsonFor(target, "bong:__stop_not_playing_probe__", 0);
        VfxEventRouter.RouteResult notPlayingResult = router.route(notPlaying, jsonLen(notPlaying));
        assertTrue(notPlayingResult.isBridgeMiss(),
            "未在播动画的 stop 应降级 bridgeMiss，实际：" + notPlayingResult.logMessage());

        UUID unresolvable = UUID.randomUUID();
        String missTarget = stopAnimJsonFor(unresolvable, "bong:__stop_no_target_probe__", 0);
        VfxEventRouter.RouteResult missResult = router.route(missTarget, jsonLen(missTarget));
        assertTrue(missResult.isBridgeMiss(),
            "目标不可解析的 stop 应降级 bridgeMiss，实际：" + missResult.logMessage());
    }

    private static String playAnimJson(String animId, int priority, int fadeInTicks) {
        return playAnimJsonFor(FIXTURE_UUID, animId, priority, fadeInTicks);
    }

    private static String playAnimJsonFor(UUID target, String animId, int priority, int fadeInTicks) {
        return """
            {"v":1,"type":"play_anim","target_player":"%s","anim_id":"%s",\
            "priority":%d,"fade_in_ticks":%d}"""
            .formatted(target, animId, priority, fadeInTicks);
    }

    /** anim_json 经 Gson 序列化转义（真实资产字节含引号/换行，手拼必坏）。 */
    private static String playAnimInlineJsonFor(
        UUID target, String animId, String animJson, int priority, int fadeInTicks) {
        JsonObject payload = new JsonObject();
        payload.addProperty("v", 1);
        payload.addProperty("type", "play_anim_inline");
        payload.addProperty("target_player", target.toString());
        payload.addProperty("anim_id", animId);
        payload.addProperty("anim_json", animJson);
        payload.addProperty("priority", priority);
        payload.addProperty("fade_in_ticks", fadeInTicks);
        return payload.toString();
    }

    private static String stopAnimJsonFor(UUID target, String animId, int fadeOutTicks) {
        return """
            {"v":1,"type":"stop_anim","target_player":"%s","anim_id":"%s",\
            "fade_out_ticks":%d}"""
            .formatted(target, animId, fadeOutTicks);
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }

    private static final class RecordingBridge implements VfxEventAnimationBridge {
        final List<PlayCall> playCalls = new ArrayList<>();
        final List<InlineCall> inlineCalls = new ArrayList<>();
        final List<StopCall> stopCalls = new ArrayList<>();
        private final boolean returnValue;

        RecordingBridge(boolean returnValue) {
            this.returnValue = returnValue;
        }

        @Override
        public boolean playAnim(UUID target, Identifier animId, int priority, OptionalInt fadeInTicks) {
            playCalls.add(new PlayCall(target, animId, priority, fadeInTicks));
            return returnValue;
        }

        @Override
        public boolean playAnimInline(
            UUID target,
            Identifier animId,
            String animJson,
            int priority,
            OptionalInt fadeInTicks
        ) {
            inlineCalls.add(new InlineCall(target, animId, animJson, priority, fadeInTicks));
            return returnValue;
        }

        @Override
        public boolean stopAnim(UUID target, Identifier animId, OptionalInt fadeOutTicks) {
            stopCalls.add(new StopCall(target, animId, fadeOutTicks));
            return returnValue;
        }

        record PlayCall(UUID target, Identifier animId, int priority, OptionalInt fadeInTicks) {
        }

        record InlineCall(UUID target, Identifier animId, String animJson, int priority, OptionalInt fadeInTicks) {
        }

        record StopCall(UUID target, Identifier animId, OptionalInt fadeOutTicks) {
        }
    }

    private static final class RecordingParticleBridge implements VfxParticleBridge {
        final List<VfxEventPayload.SpawnParticle> calls = new ArrayList<>();
        private final boolean returnValue;

        RecordingParticleBridge(boolean returnValue) {
            this.returnValue = returnValue;
        }

        @Override
        public boolean spawnParticle(VfxEventPayload.SpawnParticle payload) {
            calls.add(payload);
            return returnValue;
        }
    }

    private static final class RecordingEntityBridge implements VfxEntityAnimationBridge {
        final List<Call> calls = new ArrayList<>();
        private final boolean returnValue;

        RecordingEntityBridge(boolean returnValue) {
            this.returnValue = returnValue;
        }

        @Override
        public boolean playEntityAnim(int entityId, String anim, int durationTicks) {
            calls.add(new Call(entityId, anim, durationTicks));
            return returnValue;
        }

        record Call(int entityId, String anim, int durationTicks) {
        }
    }

    private static final class ThrowingBridge implements VfxEventAnimationBridge {
        @Override
        public boolean playAnim(UUID target, Identifier animId, int priority, OptionalInt fadeInTicks) {
            throw new IllegalStateException("simulated bridge failure");
        }

        @Override
        public boolean playAnimInline(
            UUID target,
            Identifier animId,
            String animJson,
            int priority,
            OptionalInt fadeInTicks
        ) {
            throw new IllegalStateException("simulated bridge failure");
        }

        @Override
        public boolean stopAnim(UUID target, Identifier animId, OptionalInt fadeOutTicks) {
            throw new IllegalStateException("simulated bridge failure");
        }
    }

    private static final class ThrowingEntityBridge implements VfxEntityAnimationBridge {
        @Override
        public boolean playEntityAnim(int entityId, String anim, int durationTicks) {
            throw new IllegalStateException("simulated entity bridge failure");
        }
    }
}
