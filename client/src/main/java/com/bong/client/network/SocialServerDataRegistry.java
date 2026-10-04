package com.bong.client.network;

import java.util.Map;

/** 社交、身份和关系事件的 server-data 注册表。 */
public final class SocialServerDataRegistry {
    private SocialServerDataRegistry() {
    }

    /** 注册社交事件和身份面板 consumer。 */
    public static void register(Map<String, ServerDataHandler> handlers) {
        SocialServerDataHandler social = new SocialServerDataHandler();
        handlers.put("social_anonymity", social);
        handlers.put("social_exposure", social);
        handlers.put("social_pact", social);
        handlers.put("social_feud", social);
        handlers.put("social_renown_delta", social);
        handlers.put("niche_intrusion", social);
        handlers.put("niche_guardian_fatigue", social);
        handlers.put("niche_guardian_broken", social);
        handlers.put("sparring_invite", social);
        handlers.put("trade_offer", social);
        handlers.put("identity_panel_state", new IdentityPanelStateHandler());
    }
}
