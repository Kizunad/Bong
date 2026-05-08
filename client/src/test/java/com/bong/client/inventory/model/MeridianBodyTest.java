package com.bong.client.inventory.model;

import org.junit.jupiter.api.Test;

import java.util.EnumMap;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertFalse;

/**
 * plan-meridian-severed-v1 §6 P2：MeridianBody.severedChannels() 便捷访问器
 * 给将来 hotbar 灰显逻辑用。
 */
public class MeridianBodyTest {

    private static MeridianBody buildWithDamageLevels(EnumMap<MeridianChannel, ChannelState.DamageLevel> levels) {
        EnumMap<MeridianChannel, ChannelState> channels = new EnumMap<>(MeridianChannel.class);
        for (MeridianChannel ch : MeridianChannel.values()) {
            ChannelState.DamageLevel lvl = levels.getOrDefault(ch, ChannelState.DamageLevel.INTACT);
            channels.put(ch, new ChannelState(
                ch,
                /* capacity */ 10,
                /* currentFlow */ 5,
                lvl,
                /* contamination */ 0,
                /* healProgress */ 0,
                /* blocked */ false
            ));
        }
        return MeridianBody.builder().channels(channels).build();
    }

    @Test
    void severedChannelsEmptyWhenAllIntact() {
        MeridianBody body = buildWithDamageLevels(new EnumMap<>(MeridianChannel.class));
        assertTrue(body.severedChannels().isEmpty(),
            "全部 INTACT 时 severedChannels 应返回空集，给 hotbar 灰显逻辑提供 noop 锚点");
    }

    @Test
    void severedChannelsContainsOnlySeveredOnes() {
        EnumMap<MeridianChannel, ChannelState.DamageLevel> levels = new EnumMap<>(MeridianChannel.class);
        levels.put(MeridianChannel.LU, ChannelState.DamageLevel.SEVERED);
        levels.put(MeridianChannel.HT, ChannelState.DamageLevel.TORN);
        levels.put(MeridianChannel.LI, ChannelState.DamageLevel.SEVERED);
        levels.put(MeridianChannel.SP, ChannelState.DamageLevel.MICRO_TEAR);

        MeridianBody body = buildWithDamageLevels(levels);
        Set<MeridianChannel> severed = body.severedChannels();

        assertEquals(2, severed.size(),
            "只统计 SEVERED 等级，TORN/MICRO_TEAR 不应混入");
        assertTrue(severed.contains(MeridianChannel.LU));
        assertTrue(severed.contains(MeridianChannel.LI));
        assertFalse(severed.contains(MeridianChannel.HT));
        assertFalse(severed.contains(MeridianChannel.SP));
    }

    @Test
    void severedChannelsIsImmutableSnapshot() {
        EnumMap<MeridianChannel, ChannelState.DamageLevel> levels = new EnumMap<>(MeridianChannel.class);
        levels.put(MeridianChannel.LU, ChannelState.DamageLevel.SEVERED);
        MeridianBody body = buildWithDamageLevels(levels);
        Set<MeridianChannel> first = body.severedChannels();
        // 多次调用应返回一致结果（无副作用）
        assertEquals(first, body.severedChannels());
    }
}
