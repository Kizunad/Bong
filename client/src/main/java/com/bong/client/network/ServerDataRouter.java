package com.bong.client.network;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

public final class ServerDataRouter {
    private final Map<String, ServerDataHandler> handlers;

    public ServerDataRouter(Map<String, ServerDataHandler> handlers) {
        this.handlers = Map.copyOf(handlers);
    }

    public static ServerDataRouter createDefault() {
        LegacyMessageServerDataHandler legacyHandler = new LegacyMessageServerDataHandler();
        NarrationHandler narrationHandler = new NarrationHandler();
        ZoneInfoHandler zoneInfoHandler = new ZoneInfoHandler();
        EventAlertHandler eventAlertHandler = new EventAlertHandler();
        PlayerStateHandler playerStateHandler = new PlayerStateHandler();
        UiOpenHandler uiOpenHandler = new UiOpenHandler();
        CultivationDetailHandler cultivationDetailHandler = new CultivationDetailHandler();
        InventorySnapshotHandler inventorySnapshotHandler = new InventorySnapshotHandler();
        InventoryEventHandler inventoryEventHandler = new InventoryEventHandler();
        DroppedLootSyncHandler droppedLootSyncHandler = new DroppedLootSyncHandler();
        BotanyHarvestProgressHandler botanyHarvestProgressHandler = new BotanyHarvestProgressHandler();
        BotanySkillHandler botanySkillHandler = new BotanySkillHandler();
        HeartDemonOfferHandler heartDemonOfferHandler = new HeartDemonOfferHandler();
        com.bong.client.network.alchemy.AlchemyFurnaceHandler alchemyFurnaceHandler =
            new com.bong.client.network.alchemy.AlchemyFurnaceHandler();
        com.bong.client.network.alchemy.AlchemySessionHandler alchemySessionHandler =
            new com.bong.client.network.alchemy.AlchemySessionHandler();
        com.bong.client.network.alchemy.AlchemyOutcomeForecastHandler alchemyForecastHandler =
            new com.bong.client.network.alchemy.AlchemyOutcomeForecastHandler();
        com.bong.client.network.alchemy.AlchemyRecipeBookHandler alchemyRecipeBookHandler =
            new com.bong.client.network.alchemy.AlchemyRecipeBookHandler();
        com.bong.client.network.alchemy.AlchemyContaminationHandler alchemyContaminationHandler =
            new com.bong.client.network.alchemy.AlchemyContaminationHandler();
        com.bong.client.network.alchemy.AlchemyOutcomeResolvedHandler alchemyOutcomeResolvedHandler =
            new com.bong.client.network.alchemy.AlchemyOutcomeResolvedHandler();
        com.bong.client.combat.handler.CombatEventHandler combatEventHandler =
            new com.bong.client.combat.handler.CombatEventHandler();
        com.bong.client.combat.handler.StatusSnapshotHandler statusSnapshotHandler =
            new com.bong.client.combat.handler.StatusSnapshotHandler();
        com.bong.client.combat.handler.DerivedAttrsHandler derivedAttrsHandler =
            new com.bong.client.combat.handler.DerivedAttrsHandler();
        com.bong.client.combat.handler.DeathScreenHandler deathScreenHandler =
            new com.bong.client.combat.handler.DeathScreenHandler();
        com.bong.client.combat.handler.TerminateScreenHandler terminateScreenHandler =
            new com.bong.client.combat.handler.TerminateScreenHandler();
        com.bong.client.combat.handler.WoundsSnapshotHandler woundsSnapshotHandler =
            new com.bong.client.combat.handler.WoundsSnapshotHandler();
        com.bong.client.combat.handler.TribulationBroadcastHandler tribulationBroadcastHandler =
            new com.bong.client.combat.handler.TribulationBroadcastHandler();
        com.bong.client.combat.handler.AscensionQuotaHandler ascensionQuotaHandler =
            new com.bong.client.combat.handler.AscensionQuotaHandler();
        CombatHudStateHandler combatHudStateHandler = new CombatHudStateHandler();
        DefenseWindowHandler defenseWindowHandler = new DefenseWindowHandler();
        CastSyncHandler castSyncHandler = new CastSyncHandler();
        QuickSlotConfigHandler quickSlotConfigHandler = new QuickSlotConfigHandler();
        SkillBarConfigHandler skillBarConfigHandler = new SkillBarConfigHandler();
        TechniquesSnapshotHandler techniquesSnapshotHandler = new TechniquesSnapshotHandler();
        UnlocksSyncHandler unlocksSyncHandler = new UnlocksSyncHandler();
        EventStreamPushHandler eventStreamPushHandler = new EventStreamPushHandler();
        WeaponEquippedHandler weaponEquippedHandler = new WeaponEquippedHandler();
        WeaponBrokenHandler weaponBrokenHandler = new WeaponBrokenHandler();
        TreasureEquippedHandler treasureEquippedHandler = new TreasureEquippedHandler();
        ExtractServerDataHandler extractServerDataHandler = new ExtractServerDataHandler();
        com.bong.client.network.lingtian.LingtianSessionHandler lingtianSessionHandler =
            new com.bong.client.network.lingtian.LingtianSessionHandler();

        Map<String, ServerDataHandler> handlers = new LinkedHashMap<>();
        handlers.put("welcome", legacyHandler);
        handlers.put("heartbeat", legacyHandler);
        handlers.put("narration", narrationHandler);
        handlers.put("zone_info", zoneInfoHandler);
        handlers.put("event_alert", eventAlertHandler);
        handlers.put("player_state", playerStateHandler);
        handlers.put("ui_open", uiOpenHandler);
        handlers.put("cultivation_detail", cultivationDetailHandler);
        handlers.put("inventory_snapshot", inventorySnapshotHandler);
        handlers.put("inventory_event", inventoryEventHandler);
        handlers.put("dropped_loot_sync", droppedLootSyncHandler);
        handlers.put("botany_harvest_progress", botanyHarvestProgressHandler);
        handlers.put("botany_skill", botanySkillHandler);
        handlers.put("alchemy_furnace", alchemyFurnaceHandler);
        handlers.put("alchemy_session", alchemySessionHandler);
        handlers.put("alchemy_outcome_forecast", alchemyForecastHandler);
        handlers.put("alchemy_recipe_book", alchemyRecipeBookHandler);
        handlers.put("alchemy_contamination", alchemyContaminationHandler);
        handlers.put("alchemy_outcome_resolved", alchemyOutcomeResolvedHandler);
        handlers.put("combat_event", combatEventHandler);
        handlers.put("status_snapshot", statusSnapshotHandler);
        handlers.put("derived_attrs_sync", derivedAttrsHandler);
        handlers.put("death_screen", deathScreenHandler);
        handlers.put("terminate_screen", terminateScreenHandler);
        handlers.put("wounds_snapshot", woundsSnapshotHandler);
        handlers.put("tribulation_broadcast", tribulationBroadcastHandler);
        handlers.put("ascension_quota", ascensionQuotaHandler);
        handlers.put("heart_demon_offer", heartDemonOfferHandler);
        handlers.put("combat_hud_state", combatHudStateHandler);
        handlers.put("defense_window", defenseWindowHandler);
        handlers.put("cast_sync", castSyncHandler);
        handlers.put("quickslot_config", quickSlotConfigHandler);
        handlers.put("skillbar_config", skillBarConfigHandler);
        handlers.put("techniques_snapshot", techniquesSnapshotHandler);
        handlers.put("unlocks_sync", unlocksSyncHandler);
        handlers.put("event_stream_push", eventStreamPushHandler);
        handlers.put("weapon_equipped", weaponEquippedHandler);
        handlers.put("weapon_broken", weaponBrokenHandler);
        handlers.put("treasure_equipped", treasureEquippedHandler);
        handlers.put("rift_portal_state", extractServerDataHandler);
        handlers.put("rift_portal_removed", extractServerDataHandler);
        handlers.put("extract_started", extractServerDataHandler);
        handlers.put("extract_progress", extractServerDataHandler);
        handlers.put("extract_completed", extractServerDataHandler);
        handlers.put("extract_aborted", extractServerDataHandler);
        handlers.put("extract_failed", extractServerDataHandler);
        handlers.put("tsy_collapse_started_ipc", extractServerDataHandler);
        handlers.put("tsy_collapse_started_ipc", extractServerDataHandler);
        handlers.put("lingtian_session", lingtianSessionHandler);
        // plan-forge-v1 §4 — 炼器（武器）
        com.bong.client.network.forge.ForgeStationHandler forgeStationHandler =
            new com.bong.client.network.forge.ForgeStationHandler();
        com.bong.client.network.forge.ForgeSessionHandler forgeSessionHandler =
            new com.bong.client.network.forge.ForgeSessionHandler();
        com.bong.client.network.forge.ForgeOutcomeHandler forgeOutcomeHandler =
            new com.bong.client.network.forge.ForgeOutcomeHandler();
        com.bong.client.network.forge.ForgeBlueprintBookHandler forgeBlueprintBookHandler =
            new com.bong.client.network.forge.ForgeBlueprintBookHandler();
        handlers.put("forge_station", forgeStationHandler);
        handlers.put("forge_session", forgeSessionHandler);
        handlers.put("forge_outcome", forgeOutcomeHandler);
        handlers.put("forge_blueprint_book", forgeBlueprintBookHandler);
        // plan-skill-v1 §8 — 4 个子技能事件 channel（server→client），后续各 plan 触发点接入即可吃数据
        handlers.put("skill_xp_gain", SkillEventHandler.xpGainHandler());
        handlers.put("skill_lv_up", SkillEventHandler.lvUpHandler());
        handlers.put("skill_cap_changed", SkillEventHandler.capChangedHandler());
        handlers.put("skill_scroll_used", SkillEventHandler.scrollUsedHandler());
        handlers.put("skill_snapshot", new SkillSnapshotHandler());
        return new ServerDataRouter(handlers);
    }

    public Set<String> registeredTypes() {
        return handlers.keySet();
    }

    public RouteResult route(String jsonPayload, int payloadSizeBytes) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(jsonPayload, payloadSizeBytes);
        if (!parseResult.isSuccess()) {
            return RouteResult.parseError(parseResult);
        }

        return route(parseResult.envelope());
    }

    public RouteResult route(ServerDataEnvelope envelope) {
        Objects.requireNonNull(envelope, "envelope");

        ServerDataHandler handler = handlers.get(envelope.type());
        if (handler == null) {
            return RouteResult.dispatched(
                ServerPayloadParseResult.success(envelope),
                ServerDataDispatch.noOp(
                    envelope.type(),
                    "No registered handler for payload type '" + envelope.type() + "'; payload ignored safely"
                )
            );
        }

        try {
            return RouteResult.dispatched(ServerPayloadParseResult.success(envelope), handler.handle(envelope));
        } catch (RuntimeException exception) {
            return RouteResult.dispatched(
                ServerPayloadParseResult.success(envelope),
                ServerDataDispatch.noOp(
                    envelope.type(),
                    "Handler for payload type '" + envelope.type() + "' failed safely: " + exception.getMessage()
                )
            );
        }
    }

    public static final class RouteResult {
        private final ServerPayloadParseResult parseResult;
        private final ServerDataDispatch dispatch;

        private RouteResult(ServerPayloadParseResult parseResult, ServerDataDispatch dispatch) {
            this.parseResult = parseResult;
            this.dispatch = dispatch;
        }

        private static RouteResult parseError(ServerPayloadParseResult parseResult) {
            return new RouteResult(parseResult, null);
        }

        private static RouteResult dispatched(ServerPayloadParseResult parseResult, ServerDataDispatch dispatch) {
            return new RouteResult(parseResult, dispatch);
        }

        public ServerPayloadParseResult parseResult() {
            return parseResult;
        }

        public ServerDataEnvelope envelope() {
            return parseResult.envelope();
        }

        public ServerDataDispatch dispatch() {
            return dispatch;
        }

        public boolean isParseError() {
            return !parseResult.isSuccess();
        }

        public boolean isHandled() {
            return dispatch != null && dispatch.handled();
        }

        public boolean isNoOp() {
            return dispatch != null && !dispatch.handled();
        }

        public String logMessage() {
            if (dispatch != null) {
                return dispatch.logMessage();
            }
            return parseResult.errorMessage();
        }
    }
}
