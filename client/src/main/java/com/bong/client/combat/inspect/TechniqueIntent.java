package com.bong.client.combat.inspect;

import com.bong.client.ui.contract.UiIntent;
import com.google.gson.JsonObject;

/** 功法窗口复用既有绑定和配置请求，不把窗口状态写入协议。 */
public sealed interface TechniqueIntent extends UiIntent {
    record BindChecked(boolean dash, int slot, String skillId, String expectedBinding) implements TechniqueIntent {}
    record Clear(int slot) implements TechniqueIntent {}
    record Configure(String skillId, JsonObject config) implements TechniqueIntent {
        public Configure { config = config.deepCopy(); }
        @Override public JsonObject config() { return config.deepCopy(); }
    }
}
