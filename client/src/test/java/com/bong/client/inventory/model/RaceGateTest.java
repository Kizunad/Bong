package com.bong.client.inventory.model;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 P3c — {@link RaceGate} 三档匹配 + fromWire fail-closed。
 */
class RaceGateTest {

    @Test
    void anyAllowsEverything() {
        RaceGate any = new RaceGate("any", List.of());
        assertTrue(any.allows("whale", false));
        assertTrue(any.allows("human", true));
        assertTrue(any.allows(null, false));
        assertTrue(any.isAny());
    }

    @Test
    void humanoidChecksIsHumanoidFlagOnly() {
        RaceGate humanoid = new RaceGate("humanoid", List.of());
        assertTrue(humanoid.allows("anything", true), "humanoid 只看 isHumanoid=true，不看 raceId");
        assertFalse(humanoid.allows("human", false), "isHumanoid=false → 拒绝，即便 raceId 看似人类");
        assertFalse(humanoid.isAny());
    }

    @Test
    void speciesChecksRaceIdMembership() {
        RaceGate whale = new RaceGate("species", List.of("whale", "shark"));
        assertTrue(whale.allows("whale", false), "名单内 → 放行（不看 isHumanoid）");
        assertTrue(whale.allows("shark", true));
        assertFalse(whale.allows("human", true), "名单外 → 拒绝");
        assertFalse(whale.allows(null, false), "null raceId → 拒绝（不崩）");
    }

    @Test
    void fromWireAcceptsThreeKnownKinds() {
        assertEquals("any", RaceGate.fromWire("any", List.of()).kind());
        assertEquals("humanoid", RaceGate.fromWire("humanoid", List.of()).kind());
        RaceGate sp = RaceGate.fromWire("species", List.of("whale"));
        assertEquals("species", sp.kind());
        assertEquals(List.of("whale"), sp.species());
    }

    @Test
    void fromWireUnknownKindFailClosedToNull() {
        assertNull(RaceGate.fromWire("cyborg", List.of()),
            "未知 kind 必须 fail-closed 成 null（调用方拒绝，不静默兜底 any）");
        assertNull(RaceGate.fromWire(null, List.of()), "null kind → null");
        assertNull(RaceGate.fromWire("", List.of()), "空 kind → null");
    }

    @Test
    void nullSpeciesNormalizedToEmptyList() {
        RaceGate g = new RaceGate("humanoid", null);
        assertEquals(List.of(), g.species(), "null species 归一成空 list，不 NPE");
    }

    @Test
    void unknownKindAllowsReturnsFalseDefensively() {
        // 理论上 fromWire 已拦掉未知 kind，但直接构造的防御分支必须拒绝。
        RaceGate weird = new RaceGate("bogus", List.of());
        assertFalse(weird.allows("whale", true));
        assertFalse(weird.allows("human", true));
    }
}
