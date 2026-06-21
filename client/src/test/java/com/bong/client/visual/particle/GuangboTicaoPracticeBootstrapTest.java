package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 锁死广播体操练习正反馈粒子的跨端契约——server {@code emit_guangbo_ticao_visual_triggers}
 * 发 SpawnParticle event_id = {@code VFX_GUANGBO_TICAO_PRACTICE}（即 {@code bong:guangbo_ticao_practice}），
 * client 必须能把它解析到 {@link GuangboTicaoPracticePlayer}，否则 {@code VfxRegistry} 查表 miss →
 * {@code BongVfxParticleBridge} 返回 false → {@code VfxEventRouter} 记 bridgeMiss → 粒子静默丢弃，
 * 玩家看不到任何练习正反馈（这正是 blocker 修复前的孤岛态）。
 *
 * <p>可测性边界：粒子实际渲染（{@code world.addParticle(HAPPY_VILLAGER, ...)}）需运行中的
 * {@code MinecraftClient}，headless 不可达，属人工 runClient checklist。headless 能确定验证的是
 * bridge 派发前的解析路由（{@code registry.lookup(payload.eventId())}）必命中真实注册的 player——
 * 与 {@link DuguNeedleVfxBootstrapTest} / relic reveal 同一界。
 */
public class GuangboTicaoPracticeBootstrapTest {

    /** event_id 字面值必须与 server 常量 VFX_GUANGBO_TICAO_PRACTICE 逐字符一致。 */
    @Test
    void eventIdMatchesServerEmitConstant() {
        assertEquals(
            "bong:guangbo_ticao_practice",
            GuangboTicaoPracticePlayer.EVENT_ID.toString(),
            "client EVENT_ID 必须等于 server VFX_GUANGBO_TICAO_PRACTICE（bong:guangbo_ticao_practice），"
                + "否则练习粒子跨端 event_id 错位、客户端查表 miss、粒子静默丢弃"
        );
    }

    @Test
    void bootstrapRegistersGuangboPractice() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        assertTrue(
            VfxRegistry.instance().contains(GuangboTicaoPracticePlayer.EVENT_ID),
            "guangbo_ticao_practice event_id 必须注册，否则 server emit 的粒子静默丢失"
        );
    }

    /**
     * 决定性锁住「server emit 的 SpawnParticle event_id → registry/bridge → player」这段可观察路由。
     * 用一个与 server emit_spawn_particle 形状一致的 payload（含 color/strength/count/duration）查表，
     * 必命中 {@link GuangboTicaoPracticePlayer}。对照未注册 id 必拒，证明命中不是 bridge 无脑放行。
     */
    @Test
    void serverEmittedPayloadRoutesToGuangboPlayer() {
        VfxRegistry.instance().clearForTests();
        VfxBootstrap.registerDefaults();
        VfxRegistry registry = VfxRegistry.instance();

        // 与 server emit_guangbo_ticao_visual_triggers 的 emit_spawn_particle 调用一致：
        // color #7FE38F、strength 0.6、count 6、duration 20t、origin 头部上方。
        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            GuangboTicaoPracticePlayer.EVENT_ID,
            new double[] { 3.0, 65.2, 9.0 },
            Optional.empty(),
            OptionalInt.of(0x7FE38F),
            Optional.of(0.6),
            OptionalInt.of(6),
            OptionalInt.of(20)
        );

        Optional<VfxPlayer> routed = registry.lookup(payload.eventId());
        assertTrue(
            routed.isPresent(),
            "server 发的 bong:guangbo_ticao_practice 必须解析到已注册 VfxPlayer——这正是 "
                + "BongVfxParticleBridge.lookupPlayer 派发前做的事；miss 即粒子静默不渲染（blocker 复发）"
        );
        assertTrue(
            routed.get() instanceof GuangboTicaoPracticePlayer,
            "registerDefaults() 必须把 guangbo_ticao_practice 绑到 GuangboTicaoPracticePlayer，实际 "
                + routed.get().getClass().getName()
        );

        // 对照：未注册 id 必让 bridge 拒（headless 确定可测，不依赖 MinecraftClient），证明上面的命中真实。
        BongVfxParticleBridge bridge = new BongVfxParticleBridge(registry);
        VfxEventPayload.SpawnParticle unknown = new VfxEventPayload.SpawnParticle(
            new Identifier("bong", "no_such_guangbo_event"),
            new double[] { 0, 0, 0 },
            Optional.empty(),
            OptionalInt.empty(),
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        );
        assertFalse(
            bridge.spawnParticle(unknown),
            "未注册 event_id 必让 bridge return false——反证命中是因真注册而非 bridge 无脑放行"
        );
    }
}
