package com.bong.client.network;

import java.util.Map;

/** 战斗、施法、功法和装备反馈 consumer 的分域注册表。 */
public final class CombatServerDataRegistry {
    private CombatServerDataRegistry() {
    }

    /** 注册战斗与 HUD 状态 handler。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        handlers.put("breakthrough_cinematic", new BreakthroughCinematicHandler());
        handlers.put("combat_event", new com.bong.client.combat.handler.CombatEventHandler());
        handlers.put("knockback_sync", new KnockbackSyncHandler());
        handlers.put("status_snapshot", new com.bong.client.combat.handler.StatusSnapshotHandler());
        handlers.put("derived_attrs_sync", new com.bong.client.combat.handler.DerivedAttrsHandler());
        handlers.put("vortex_state", new com.bong.client.combat.handler.VortexStateHandler());
        handlers.put("dugu_poison_state", new com.bong.client.combat.handler.DuguPoisonStateHandler());

        PoisonTraitServerDataHandler poison = new PoisonTraitServerDataHandler();
        handlers.put("poison_dose_event", poison);
        handlers.put("poison_overdose_event", poison);
        handlers.put("poison_trait_state", poison);
        handlers.put("carrier_state", new com.bong.client.combat.handler.CarrierStateHandler());
        handlers.put("false_skin_state", new com.bong.client.combat.handler.FalseSkinStateHandler());
        handlers.put("death_screen", new com.bong.client.combat.handler.DeathScreenHandler());
        handlers.put("terminate_screen", new com.bong.client.combat.handler.TerminateScreenHandler());
        handlers.put("wounds_snapshot", new com.bong.client.combat.handler.WoundsSnapshotHandler());
        handlers.put("tribulation_state", new com.bong.client.combat.handler.TribulationStateHandler());
        handlers.put("tribulation_broadcast", new com.bong.client.combat.handler.TribulationBroadcastHandler());
        handlers.put("ascension_quota", new com.bong.client.combat.handler.AscensionQuotaHandler());
        handlers.put("heart_demon_offer", new HeartDemonOfferHandler());
        handlers.put("combat_hud_state", new CombatHudStateHandler());
        handlers.put("defense_window", new DefenseWindowHandler());
        handlers.put("cast_sync", new CastSyncHandler());
        handlers.put("quickslot_config", new QuickSlotConfigHandler());
        handlers.put("skillbar_config", new SkillBarConfigHandler());
        handlers.put("techniques_snapshot", new TechniquesSnapshotHandler());
        handlers.put("technique_proficiency_update", new TechniqueProficiencyUpdateHandler());
        handlers.put("skill_config_snapshot", new SkillConfigSnapshotHandler());
        handlers.put("unlocks_sync", new UnlocksSyncHandler());
        handlers.put("event_stream_push", new EventStreamPushHandler());
        handlers.put("burst_meridian_event", new BurstMeridianHandler());
        handlers.put("pill_buff_status", new PillBuffStatusHandler());
        handlers.put("skill_xp_gain", SkillEventHandler.xpGainHandler());
        handlers.put("skill_lv_up", SkillEventHandler.lvUpHandler());
        handlers.put("skill_cap_changed", SkillEventHandler.capChangedHandler());
        handlers.put("skill_scroll_used", SkillEventHandler.scrollUsedHandler());
        handlers.put("skill_snapshot", new SkillSnapshotHandler());

        FullPowerStateHandler fullPower = new FullPowerStateHandler();
        handlers.put("full_power_charging_state", fullPower);
        handlers.put("full_power_release", fullPower);
        handlers.put("full_power_exhausted_state", fullPower);
        handlers.put("weapon_equipped", new WeaponEquippedHandler());
        handlers.put("weapon_broken", new WeaponBrokenHandler());
        handlers.put("shield_broken", new ShieldBrokenHandler());
        handlers.put("shield_block_hit", new ShieldBlockHitHandler());
        handlers.put("treasure_equipped", new TreasureEquippedHandler());
    }
}
