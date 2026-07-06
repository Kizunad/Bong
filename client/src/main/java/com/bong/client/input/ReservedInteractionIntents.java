package com.bong.client.input;

public final class ReservedInteractionIntents {
    public static final int OPEN_CONTAINER_PRIORITY = 100;
    public static final int SEARCH_CONTAINER_PRIORITY = 100;
    public static final int TRADE_PLAYER_PRIORITY = 90;
    public static final int TALK_NPC_PRIORITY = 90;
    public static final int ACTIVATE_SHRINE_PRIORITY = 80;
    public static final int PICKUP_DROPPED_ITEM_PRIORITY = 70;
    /** plan-remains-suite P0 — 遗骸 G 键拾取，与地面掉落物同一优先级层（同层按距离 tie-break）。 */
    public static final int LOOT_REMAINS_PRIORITY = 70;
    public static final int HARVEST_RESOURCE_PRIORITY = 60;

    private ReservedInteractionIntents() {
    }
}
