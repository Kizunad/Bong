package com.bong.client.ui.preview;

import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.texture.NativeImage;

/** 本地固定 UI 场景；不读取网络、不接受动态类名。 */
interface UiPreviewScene {
    void installFixture();

    Screen createScreen();

    /** 等屏幕过渡和初始化完成后，挂载仅属于当前屏幕的预览行为。 */
    default void afterOpen(Screen screen) {}

    String selectedTemplateId(Screen screen);

    boolean isReady(Screen screen);

    boolean initializationFailed(Screen screen);

    void validateGeometry(Screen screen, UiPreviewShot shot);

    /** 可选的真实像素门禁，不能仅以命令列表非空推断绘制成功。 */
    default void validateImage(Screen screen, NativeImage image) {}

    void cleanup();
}
