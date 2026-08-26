package com.bong.client.ui.bootstrap;

import java.util.Set;

/** 显式的 UI/HUD/keybind 启动单元。 */
public interface UiBootstrapModule {
    String id();

    Set<String> dependencies();

    void register(UiRuntime runtime);
}
