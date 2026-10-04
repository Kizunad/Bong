package com.bong.client.network;

import java.util.Map;

/**
 * 注册玩家连接、基础状态和库存快照相关的 server-data consumer。
 *
 * <p>这里只建立 type 到既有 handler 的映射，不改变 payload 的解析、生命周期或
 * store 写入语义。新的 wire contract 在 production activation 前不得在此处接线。</p>
 */
public final class CoreServerDataRegistry {
    private CoreServerDataRegistry() {
    }

    /** 将核心状态 handler 注册到默认路由表。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        LegacyMessageServerDataHandler legacy = new LegacyMessageServerDataHandler();
        handlers.put("welcome", legacy);
        handlers.put("heartbeat", legacy);
        handlers.put("narration", new NarrationHandler());
        handlers.put("zone_info", new ZoneInfoHandler());
        handlers.put("event_alert", new EventAlertHandler());
        handlers.put("player_state", new PlayerStateHandler());
        handlers.put("ui_open", new UiOpenHandler());
        handlers.put("cultivation_detail", new CultivationDetailHandler());
        handlers.put("body_plan_layout", new BodyPlanLayoutHandler());
        handlers.put("race_gate_meta", new RaceGateMetaHandler());
        handlers.put("morph_state", new MorphStateHandler());
        handlers.put("qi_color_observed", new QiColorObservedHandler());
        handlers.put("inventory_snapshot", new InventorySnapshotHandler());
        handlers.put("inventory_event", new InventoryEventHandler());
        handlers.put("dropped_loot_sync", new DroppedLootSyncHandler());
        handlers.put("remains_sync", new RemainsSyncHandler());
    }
}
