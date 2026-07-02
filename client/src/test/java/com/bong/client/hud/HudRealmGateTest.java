package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-wire-format-bridge-v1 P1／RC2 crit — {@code player_state.realm} 枚举前缀 fixup 的
 * 消费端锁死测试。
 * <p>
 * {@code ProtoServerDataBridge.bridgePlayerState} 把 proto 枚举全名 "REALM_CONDENSE" 剥成
 * "Condense"（{@code normalizeRealmField} 产出格式，与 {@code bridgeCultivationDetail} 一致）。
 * 这里直接钉住 {@link HudRealmGate} 收到这种格式后的境界门控判定——喂 "Condense" 必须判凝脉
 * （tier 2），而非桥修复前 PLAYER_STATE 走 generic path 时的恒定醒灵（tier 0）。
 */
class HudRealmGateTest {

    @Test
    void bridgedCondenseRealmResolvesToTierTwoNotAwaken() {
        assertEquals(2, HudRealmGate.tier("Condense"),
                "喂 bridgePlayerState 产出的 'Condense'（剥前缀+首字母大写格式）必须判凝脉（tier "
                + "2），修复前 PLAYER_STATE 走 generic path 不剥前缀，'REALM_CONDENSE' 落在 tier() "
                + "switch 的 default 分支恒判醒灵（tier 0），所有境界门控 HUD 永不解锁");
        assertTrue(HudRealmGate.atLeastCondense("Condense"));
        assertFalse(HudRealmGate.atLeastSpirit("Condense"));
    }

    @Test
    void allSixBridgedRealmLiteralsResolveToDistinctTiers() {
        record Case(String bridged, int expectedTier) {}
        Case[] cases = new Case[] {
            new Case("Awaken", 0),
            new Case("Induce", 1),
            new Case("Condense", 2),
            new Case("Solidify", 3),
            new Case("Spirit", 4),
            new Case("Void", 5),
        };
        for (Case c : cases) {
            assertEquals(c.expectedTier(), HudRealmGate.tier(c.bridged()),
                    "bridged realm literal '" + c.bridged() + "' should resolve to tier "
                    + c.expectedTier());
        }
    }

    @Test
    void unbridgedRawProtoEnumNameFallsBackToAwakenTier() {
        // 修复前的回归防线：万一将来某处又漏接 normalizeRealmField，裸 proto 常量必须
        // 可预期地回落 tier 0（而不是抛异常），同时这条测试本身就是"这是坏值"的显式记录。
        assertEquals(0, HudRealmGate.tier("REALM_CONDENSE"),
                "未剥前缀的裸 proto 常量必须回落 tier 0（醒灵）——这正是 PLAYER_STATE 修复前的"
                + "实际线上行为，证明 bridgePlayerState 的 normalizeRealmField 接线是必要的");
    }
}
