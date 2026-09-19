package com.bong.client.ui.preview;

import net.minecraft.client.gui.screen.Screen;

/** 本地固定 UI 场景；不读取网络、不接受动态类名。 */
interface UiPreviewScene {
    void installFixture(UiPreviewConfig config);

    Screen createScreen();

    String selectedTemplateId(Screen screen);

    boolean isReady(Screen screen);

    boolean initializationFailed(Screen screen);

    /** 在截图等待阶段前完成输入，截图读取后续正常渲染帧。 */
    default void prepareScreenshot(Screen screen, UiPreviewShot shot) {}

    void validateGeometry(Screen screen, UiPreviewShot shot);

    void cleanup();
}
