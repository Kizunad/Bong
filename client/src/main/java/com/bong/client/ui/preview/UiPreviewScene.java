package com.bong.client.ui.preview;

import net.minecraft.client.gui.screen.Screen;

/** 本地登记的 UI 验收场景，不接受动态类名。 */
interface UiPreviewScene {
    /** 实体模型场景需要真实 ClientWorld；普通 UI 场景仍可在标题页验证。 */
    default boolean clientReady(net.minecraft.client.MinecraftClient client) { return true; }

    void installFixture(UiPreviewConfig config);

    Screen createScreen();

    String selectedTemplateId(Screen screen);

    boolean isReady(Screen screen);

    boolean initializationFailed(Screen screen);

    /** 在截图等待阶段前完成输入，截图读取后续正常渲染帧。 */
    default void prepareScreenshot(Screen screen, UiPreviewShot shot) {}

    /** 等待截图期间维持显式夹具；不注册长期网络回调。 */
    default void tick() {}

    void validateGeometry(Screen screen, UiPreviewShot shot);

    void cleanup();
}
