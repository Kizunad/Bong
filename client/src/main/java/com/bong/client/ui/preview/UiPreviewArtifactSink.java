package com.bong.client.ui.preview;

import net.minecraft.client.texture.NativeImage;

import java.io.IOException;

/** UI preview 产物写入边界；状态机不直接依赖本地文件系统。 */
interface UiPreviewArtifactSink {
    void begin() throws IOException;

    String writeShot(String shotName, NativeImage image, String metadata) throws IOException;

    void passed(int completed) throws IOException;

    void failed(int completed, String phase, String shot, Throwable failure) throws IOException;
}
