package com.bong.client;

public final class BongClientFeatures {
    public static final boolean ENABLE_TOASTS = true;
    public static final boolean ENABLE_VISUAL_EFFECTS = true;
    // raw XML UI must stay disabled until explicitly enabled — agent-ui 走 AgentUiPayloadHandler 不读此 flag
    public static final boolean ENABLE_DYNAMIC_XML_UI = false;
    public static final boolean ENABLE_XML_TEMPLATE_MODE = true;
    public static final boolean ENABLE_DEBUG_HEARTBEAT_CHAT = false;
    public static final boolean ENABLE_COMBAT_HUD = true;
    public static final boolean ENABLE_BOTANY_HUD = true;
    public static final boolean ENABLE_UI_TRANSITIONS = true;

    private BongClientFeatures() {
    }
}
