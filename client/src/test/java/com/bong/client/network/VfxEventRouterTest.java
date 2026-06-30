package com.bong.client.network;

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
