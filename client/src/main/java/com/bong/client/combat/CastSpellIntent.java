package com.bong.client.combat;

/** Release a configured spell volume (§11.3). */
public record CastSpellIntent(float radius, float velocityCap, float qiInvest) {
    public static CastSpellIntent fromVolume(SpellVolumeState state) {
        if (state == null) return new CastSpellIntent(SpellVolumeState.MIN_RADIUS, SpellVolumeState.MIN_VELOCITY, 0.0f);
        return new CastSpellIntent(state.radius(), state.velocityCap(), state.qiInvest());
    }
}
