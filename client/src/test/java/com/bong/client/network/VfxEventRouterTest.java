package com.bong.client.network;

import com.bong.client.animation.AnimWiringManifestTest;
import com.bong.client.animation.AnimationLayerManager;
import com.bong.client.animation.ClientAnimationBridge;
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
     * 未知 anim_id 失败分支：bridge 委托真实 {@link AnimationLayerManager#playOnStack}
     * （生产 {@link ClientAnimationBridge#playAnim} 在解析到玩家后走的同一注册表查询路径），
     * 注册表查不到的 anim id → play 返回 false → 路由记 bridgeMiss，不抛异常不崩溃。
     */
    @Test
    void unknownAnimIdBecomesBridgeMissNotCrashThroughLayerManagerLookup() throws IOException {
        LayerManagerLookupBridge bridge = new LayerManagerLookupBridge();
        VfxEventRouter router = new VfxEventRouter(bridge);
        String json = playAnimJson("bong:__vfx_router_unknown_anim__", 1000, 3);

        VfxEventRouter.RouteResult result = router.route(json, jsonLen(json));

        assertEquals(1, bridge.invocations, "bridge 应被真实调用到（未知 id 在注册表层才失败）");
        assertTrue(result.isBridgeMiss(),
            "未知 anim id 应走注册表 miss → bridgeMiss 降级，实际：" + result.logMessage());
        assertFalse(result.isHandled(),
            "未知 anim id 绝不能被标记为已处理（否则缺资产静默上线）");
    }

    private static String playAnimJson(String animId, int priority, int fadeInTicks) {
        return """
            {"v":1,"type":"play_anim","target_player":"%s","anim_id":"%s",\
            "priority":%d,"fade_in_ticks":%d}"""
            .formatted(FIXTURE_UUID, animId, priority, fadeInTicks);
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }

    /** 复刻生产 playAnim 的注册表查询语义（跳过需 MC 运行时的玩家实体解析）。 */
    private static final class LayerManagerLookupBridge implements VfxEventAnimationBridge {
        int invocations;

        @Override
        public boolean playAnim(UUID target, Identifier animId, int priority, OptionalInt fadeInTicks) {
            invocations++;
            return AnimationLayerManager.playOnStack(
                new AnimationStack(),
                target,
                AnimationLayerManager.channelForPriority(priority),
                animId
            );
        }

        @Override
        public boolean playAnimInline(
            UUID target,
            Identifier animId,
            String animJson,
            int priority,
            OptionalInt fadeInTicks
        ) {
            return false;
        }

        @Override
        public boolean stopAnim(UUID target, Identifier animId, OptionalInt fadeOutTicks) {
            return false;
        }
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
