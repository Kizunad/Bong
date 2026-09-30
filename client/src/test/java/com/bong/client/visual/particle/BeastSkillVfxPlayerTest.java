package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** headless 可验证的五招粒子形态契约；真实 particle manager 画面仍由 runClient 验收。 */
public class BeastSkillVfxPlayerTest {

    @Test
    void eachBeastSkillUsesItsOwnParticlePrimitive() {
        assertPrimitive(BeastSkillVfxPlayer.LION_POUNCE, SkillParticleSpawn.OrbitPoint.class, 12);
        assertPrimitive(BeastSkillVfxPlayer.LION_REND, SkillParticleSpawn.Point.class, 10);
        assertPrimitive(BeastSkillVfxPlayer.VULTURE_DIVE, SkillParticleSpawn.Ribbon.class, 8);
        assertPrimitive(BeastSkillVfxPlayer.HORSE_TRAMPLE, SkillParticleSpawn.Decal.class, 12);
        assertPrimitive(BeastSkillVfxPlayer.HORSE_KICK, SkillParticleSpawn.Pulse.class, 8);
    }

    @Test
    void directionalDiveAndKickFollowServerDirection() {
        double[] forward = { 0.0, 0.0, 1.0 };

        List<SkillParticleSpawn> dive = BeastSkillVfxPlayer.plan(
            payload(BeastSkillVfxPlayer.VULTURE_DIVE, forward, OptionalInt.of(3), OptionalInt.of(10))
        );
        assertTrue(dive.stream().allMatch(spawn -> spawn instanceof SkillParticleSpawn.Ribbon
                && spawn.velocityZ() > 0.0),
            "vulture dive ribbons must move along the caster→target direction, not use a fixed axis");

        List<SkillParticleSpawn> kick = BeastSkillVfxPlayer.plan(
            payload(BeastSkillVfxPlayer.HORSE_KICK, forward, OptionalInt.of(2), OptionalInt.of(10))
        );
        assertTrue(kick.stream().allMatch(spawn -> spawn instanceof SkillParticleSpawn.Pulse
                && spawn.velocityZ() > 0.0),
            "horse kick pulses must move along the caster→target direction, not use a radial dust burst");
    }

    @Test
    void countAndLifetimeInputsAreClampedToSafeBounds() {
        List<SkillParticleSpawn> tooFew = BeastSkillVfxPlayer.plan(
            payload(BeastSkillVfxPlayer.HORSE_TRAMPLE, null, OptionalInt.of(0), OptionalInt.of(0))
        );
        assertEquals(6, tooFew.size(),
            "ground impact needs at least six decals to read as a ring when server count is zero");
        assertTrue(tooFew.stream().allMatch(spawn -> spawn.maxAge() == 1),
            "duration=0 must still produce a one-tick particle rather than a dead particle");

        List<SkillParticleSpawn> tooMany = BeastSkillVfxPlayer.plan(
            payload(BeastSkillVfxPlayer.LION_REND, null, OptionalInt.of(500), OptionalInt.of(700))
        );
        assertEquals(48, tooMany.size(), "particle count must be capped at the shared player limit");
        assertTrue(tooMany.stream().allMatch(spawn -> spawn.maxAge() == 600),
            "duration above the shared player limit must clamp to 600 ticks");
    }

    @Test
    void unknownEventDoesNotProduceParticles() {
        List<SkillParticleSpawn> plan = BeastSkillVfxPlayer.plan(
            payload(new Identifier("bong", "not_a_beast_skill"), null,
                OptionalInt.empty(), OptionalInt.empty())
        );
        assertFalse(plan.iterator().hasNext(),
            "an event not owned by this player must not silently render a beast effect");
    }

    private static void assertPrimitive(
        Identifier eventId,
        Class<?> expectedPrimitive,
        int expectedCount
    ) {
        List<SkillParticleSpawn> plan = BeastSkillVfxPlayer.plan(
            payload(eventId, null, OptionalInt.empty(), OptionalInt.empty())
        );
        assertEquals(expectedCount, plan.size(), "default count must remain pinned for " + eventId);
        assertTrue(plan.stream().allMatch(expectedPrimitive::isInstance),
            eventId + " must use " + expectedPrimitive.getSimpleName()
                + " rather than a shared generic dust shape");
    }

    private static VfxEventPayload.SpawnParticle payload(
        Identifier eventId,
        double[] direction,
        OptionalInt count,
        OptionalInt duration
    ) {
        return new VfxEventPayload.SpawnParticle(
            eventId,
            new double[] { 10.0, 64.0, -3.0 },
            direction == null ? Optional.empty() : Optional.of(direction),
            OptionalInt.empty(),
            Optional.empty(),
            count,
            duration
        );
    }
}
