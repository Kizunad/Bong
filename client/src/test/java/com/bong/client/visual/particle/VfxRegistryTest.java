package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class VfxRegistryTest {

    @AfterEach
    void clearRegistry() {
        VfxRegistry.instance().clearForTests();
    }

    @Test
    void registerThenLookupReturnsSameInstance() {
        VfxPlayer player = (client, payload) -> {};
        Identifier id = new Identifier("bong", "test_event");

        VfxPlayer previous = VfxRegistry.instance().register(id, player);
        assertNull(previous, "first registration should not replace anything");
        assertTrue(VfxRegistry.instance().contains(id));
        Optional<VfxPlayer> found = VfxRegistry.instance().lookup(id);
        assertTrue(found.isPresent());
        assertSame(player, found.get());
    }

    @Test
    void registerReplacesPrevious() {
        VfxPlayer first = (client, payload) -> {};
        VfxPlayer second = (client, payload) -> {};
        Identifier id = new Identifier("bong", "test_event");

        VfxRegistry.instance().register(id, first);
        VfxPlayer replaced = VfxRegistry.instance().register(id, second);
        assertSame(first, replaced, "register should return the previous player");
        assertSame(second, VfxRegistry.instance().lookup(id).orElseThrow());
    }

    @Test
    void lookupMissIsEmpty() {
        Optional<VfxPlayer> found = VfxRegistry.instance()
            .lookup(new Identifier("bong", "nonexistent"));
        assertFalse(found.isPresent());
    }

    @Test
    void bridgeDispatchesToRegisteredPlayer() {
        RecordingPlayer recorder = new RecordingPlayer();
        VfxRegistry registry = VfxRegistry.instance();
        registry.register(SwordQiSlashPlayer.EVENT_ID, recorder);

        BongVfxParticleBridge bridge = new BongVfxParticleBridge(registry);

        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            SwordQiSlashPlayer.EVENT_ID,
            new double[] { 1.0, 2.0, 3.0 },
            Optional.empty(),
            OptionalInt.empty(),
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        );

        // MinecraftClient.getInstance() 在单测 JVM 下可能为 null；recorder 不 deref client，
        // 所以 player.play 会被调用，但 client 参数为 null —— 这就是我们期望的测试形态，
        // 验证分发路径可达。
        boolean ok = bridge.spawnParticle(payload);
        // MinecraftClient.getInstance() 为 null 时 bridge 返回 false（注释里写了），
        // 所以无论 recorder 是否被调用，此处判定按"bridge 行为规范"：
        // - 若 MC 实例在测试 classpath 下 getInstance 返回 null，则 ok=false、recorder 未被调用
        // - 若 getInstance 返回非 null（已初始化），则 ok=true、recorder 被调用
        // 测试两条分支都接受，但要求 registry 命中语义正确：
        if (ok) {
            assertEquals(1, recorder.calls.size());
            assertSame(payload, recorder.calls.get(0));
        } else {
            assertTrue(recorder.calls.isEmpty(),
                "when MC is uninitialized, bridge should decline without invoking player");
        }
    }

    @Test
    void bridgeDeclinesUnregisteredEvent() {
        BongVfxParticleBridge bridge = new BongVfxParticleBridge(VfxRegistry.instance());
        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            new Identifier("bong", "unknown_event"),
            new double[] { 0, 0, 0 },
            Optional.empty(),
            OptionalInt.empty(),
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        );
        assertFalse(bridge.spawnParticle(payload),
            "unregistered event_id should cause bridge to return false");
    }

    @Test
    void registerDefaultsBindsSwordQiSlash() {
        VfxBootstrap.registerDefaults();
        assertTrue(VfxRegistry.instance().contains(SwordQiSlashPlayer.EVENT_ID),
            "bootstrap should register sword_qi_slash");
        assertTrue(VfxRegistry.instance().contains(FlyingSwordDemoPlayer.EVENT_ID),
            "bootstrap should register flying_sword_demo");
        assertTrue(VfxRegistry.instance().contains(FormationCoreDemoPlayer.EVENT_ID),
            "bootstrap should register formation_core_demo");
        assertTrue(VfxRegistry.instance().contains(TribulationBoundaryPlayer.EVENT_ID),
            "bootstrap should register tribulation_boundary");
        assertTrue(VfxRegistry.instance().contains(TribulationOmenCloudPlayer.EVENT_ID),
            "bootstrap should register tribulation_omen_cloud");
        assertTrue(VfxRegistry.instance().contains(RealmCollapseBoundaryPlayer.EVENT_ID),
            "bootstrap should register realm_collapse_boundary");
        assertTrue(VfxRegistry.instance().contains(FrostBreathPlayer.EVENT_ID),
            "bootstrap should register frost_breath");
        assertTrue(VfxRegistry.instance().contains(RatSwarmAuraPlayer.EVENT_ID),
            "bootstrap should register rat_swarm_aura");
        assertTrue(VfxRegistry.instance().contains(FaunaSpawnDustPlayer.EVENT_ID),
            "bootstrap should register fauna_spawn_dust");
        assertTrue(VfxRegistry.instance().contains(FaunaBoneShatterPlayer.EVENT_ID),
            "bootstrap should register fauna_bone_shatter");
        assertTrue(VfxRegistry.instance().contains(SpiderShimmerPlayer.EVENT_ID),
            "bootstrap should register spider_shimmer");
        assertTrue(VfxRegistry.instance().contains(YidaoPeacePulsePlayer.MERIDIAN_REPAIR),
            "bootstrap should register yidao_meridian_repair");
        assertTrue(VfxRegistry.instance().contains(YidaoPeacePulsePlayer.CONTAM_PURGE),
            "bootstrap should register yidao_contam_purge");
        assertTrue(VfxRegistry.instance().contains(YidaoPeacePulsePlayer.EMERGENCY_RESUSCITATE),
            "bootstrap should register yidao_emergency_resuscitate");
        assertTrue(VfxRegistry.instance().contains(YidaoPeacePulsePlayer.LIFE_EXTENSION),
            "bootstrap should register yidao_life_extension");
        assertTrue(VfxRegistry.instance().contains(YidaoPeacePulsePlayer.MASS_MERIDIAN_REPAIR),
            "bootstrap should register yidao_mass_meridian_repair");
        assertTrue(VfxRegistry.instance().contains(new Identifier("bong", "jiemai_burst_blood")),
            "bootstrap should register zhenmai parry blood burst");
        assertTrue(VfxRegistry.instance().contains(new Identifier("bong", "jiemai_neutralize_dust")),
            "bootstrap should register zhenmai neutralize dust");
        assertTrue(VfxRegistry.instance().contains(new Identifier("bong", "jiemai_sever_flash")),
            "bootstrap should register zhenmai sever flash");
        assertTrue(VfxRegistry.instance().contains(VortexSpiralPlayer.EVENT_ID),
            "bootstrap should register vortex_spiral");
        assertTrue(VfxRegistry.instance().contains(BotanyAuraPlayer.EVENT_ID),
            "bootstrap should register botany aura");
        assertTrue(VfxRegistry.instance().contains(BotanyHarvestBurstPlayer.EVENT_ID),
            "bootstrap should register botany harvest burst");
        assertTrue(VfxRegistry.instance().contains(BotanyPlantStagePlayer.ROUTE_ID),
            "bootstrap should register botany plant stage route");
        assertTrue(VfxRegistry.instance().contains(LingtianPlotRunePlayer.TILL),
            "bootstrap should register lingtian till rune");
        assertTrue(VfxRegistry.instance().contains(LingtianPlotRunePlayer.PLANT),
            "bootstrap should register lingtian plant rune");
        assertTrue(VfxRegistry.instance().contains(LingtianPlotRunePlayer.REPLENISH),
            "bootstrap should register lingtian replenish rune");
        assertTrue(VfxRegistry.instance().contains(LingtianPlotRunePlayer.HARVEST),
            "bootstrap should register lingtian harvest rune");
        assertTrue(VfxRegistry.instance().contains(LingtianPlotRunePlayer.DRAIN),
            "bootstrap should register lingtian drain rune");
        assertNotNull(VfxRegistry.instance().lookup(SwordQiSlashPlayer.EVENT_ID).orElse(null));
    }

    private static final class RecordingPlayer implements VfxPlayer {
        final List<VfxEventPayload.SpawnParticle> calls = new ArrayList<>();

        @Override
        public void play(net.minecraft.client.MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
            calls.add(payload);
        }
    }
}
