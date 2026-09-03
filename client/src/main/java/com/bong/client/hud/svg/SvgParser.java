package com.bong.client.hud.svg;

import java.io.IOException;
import java.io.InputStream;

/**
 * SVG 解析器的替换边界。
 *
 * <p>调用方只依赖受约束的 {@link SvgDocument}，不感知 StAX、JNI 或 NanoSVG
 * 的具体实现。当前 P4 slice 使用安全的 Java 解析实现；native provider 接入时
 * 只替换实现，不改变资源缓存、三角化和 GUI 提交层。</p>
 */
@FunctionalInterface
public interface SvgParser {
    SvgDocument parse(InputStream input) throws IOException;
}
