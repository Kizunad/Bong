package com.bong.client.ui.preview;

import net.minecraft.client.gui.screen.Screen;

/** 本地固定 UI 场景；不读取网络、不接受动态类名。 */
interface UiPreviewScene {
    void installFixture();

    Screen createScreen();

    String selectedTemplateId(Screen screen);

    boolean isReady(Screen screen);

    boolean initializationFailed(Screen screen);

    void validateGeometry(Screen screen, UiPreviewShot shot);

    void cleanup();
}
