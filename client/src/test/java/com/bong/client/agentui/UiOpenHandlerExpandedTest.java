package com.bong.client.agentui;

import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.UiOpenHandler;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-agent-ui-data-v1 P1 — UiOpenHandler 放宽限制验证（从 agentui 包访问 network）。
 *
 * <p>验证：
 * <ul>
 *   <li>{@code <button>} 元素允许（agent_ui XML 必用）</li>
 *   <li>{@code <grid-layout>} 元素允许</li>
 *   <li>{@code <texture>} 元素允许</li>
 *   <li>button 的 id / style / color 属性允许</li>
 *   <li>flow-layout 的 direction / gap 属性允许</li>
 *   <li>label 的 style / color / font 属性允许</li>
 *   <li>超 8192 字节→拒绝；旧 512B XML 仍通过（向后兼容）</li>
 *   <li>DOCTYPE / &lt;script&gt; 仍拒绝</li>
 * </ul>
 *
 * <p>注：{@link UiOpenHandler} 包私有构造函数/常量，通过 package-info 访问；
 * 如需访问常量，通过间接断言验证行为（不直接读取字段）。
 */
public class UiOpenHandlerExpandedTest {

    // 使用 package-private 二参构造（从 network 包 test 类可以，但此处使用 createDefault 相当于 true/true）
    // UiOpenHandler 包私有；从 agentui 包借助 ServerDataRouter 的 createDefault 测试。
    // 为了精准测试 agent_ui 场景，使用反射或单独创建默认实例（反射非最佳）。
    // 设计：重新放 UiOpenHandlerExpandedTest 到 network 包（但文件已在 agentui）
    // 更简单的方案：直接用 public 构造函数。查看 UiOpenHandler 的 public 构造：
    // UiOpenHandler() 是 public，但 UiOpenHandler(boolean, boolean) 是 package-private。
    // 公共构造 = BongClientFeatures.ENABLE_XML_TEMPLATE_MODE, BongClientFeatures.ENABLE_DYNAMIC_XML_UI
    // plan P1 已将 ENABLE_DYNAMIC_XML_UI=true，所以 new UiOpenHandler() 就是 (true, true) 模式。
    private final UiOpenHandler handler = new UiOpenHandler();

    // ─── button 元素允许 ─────────────────────────────────────────────────────

    @Test
    void buttonElement_isAllowed() {
        String xml = "<owo-ui><components>"
            + "<flow-layout><button id=\"enter_realm\">踏入</button></flow-layout>"
            + "</components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertTrue(dispatch.handled(),
            "<button> 元素应被允许（plan-agent-ui-data-v1 P1 放宽），实际 logMsg=" + dispatch.logMessage());
    }

    @Test
    void buttonElement_withStyleAndColor_isAllowed() {
        String xml = "<owo-ui><components>"
            + "<flow-layout><button id=\"dismiss\" style=\"color:#AAAAAA\">关闭</button></flow-layout>"
            + "</components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertTrue(dispatch.handled(),
            "<button style> 应被允许，实际 logMessage=" + dispatch.logMessage());
    }

    // ─── grid-layout 元素允许 ────────────────────────────────────────────────

    @Test
    void gridLayoutElement_isAllowed() {
        String xml = "<owo-ui><components>"
            + "<grid-layout columns=\"2\" rows=\"1\"><label>项一</label><label>项二</label></grid-layout>"
            + "</components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertTrue(dispatch.handled(),
            "<grid-layout> 元素应被允许，实际=" + dispatch.handled());
    }

    // ─── texture 元素允许 ────────────────────────────────────────────────────

    @Test
    void textureElement_isAllowed() {
        String xml = "<owo-ui><components>"
            + "<flow-layout><texture id=\"bong:textures/test\"/></flow-layout>"
            + "</components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertTrue(dispatch.handled(),
            "<texture> 元素应被允许，实际=" + dispatch.handled());
    }

    // ─── flow-layout 属性允许 ────────────────────────────────────────────────

    @Test
    void flowLayout_directionAndGap_isAllowed() {
        String xml = "<owo-ui><components>"
            + "<flow-layout direction=\"vertical\" gap=\"4\"><label>内容</label></flow-layout>"
            + "</components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertTrue(dispatch.handled(),
            "flow-layout 的 direction/gap 属性应被允许，实际=" + dispatch.handled());
    }

    // ─── label 属性允许 ──────────────────────────────────────────────────────

    @Test
    void label_styleColorFont_isAllowed() {
        String xml = "<owo-ui><components>"
            + "<flow-layout>"
            + "<label style=\"color:#C8A060\" font=\"bold\">天道信息</label>"
            + "</flow-layout>"
            + "</components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertTrue(dispatch.handled(),
            "label 的 style/font 属性应被允许，实际=" + dispatch.handled());
    }

    // ─── 大小边界（行为断言，不直接读取常量）─────────────────────────────────

    @Test
    void xmlOf512Bytes_stillPasses_backwardCompatible() {
        // 旧 512B 上限内的 XML 仍通过（向后兼容）
        String shortXml = "<owo-ui><components><flow-layout><label>测试</label></flow-layout></components></owo-ui>";
        assertTrue(shortXml.getBytes(StandardCharsets.UTF_8).length <= 512,
            "短 XML 应不超过 512 字节（测试前置条件）");

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(shortXml));

        assertTrue(dispatch.handled(),
            "512B 内的 XML 应仍通过（向后兼容），实际=" + dispatch.handled());
    }

    @Test
    void xmlJustUnder8192Bytes_passes() {
        // 构造接近 8192B 的合法 XML（确实通过）
        String prefix = "<owo-ui><components><flow-layout><label id=\"a\">";
        String suffix = "</label></flow-layout></components></owo-ui>";
        int baseLen = prefix.getBytes(StandardCharsets.UTF_8).length
            + suffix.getBytes(StandardCharsets.UTF_8).length;
        int fill = 8192 - baseLen - 1; // 留 1 字节余量确保 < 8192
        if (fill <= 0) {
            // XML 基础结构超长，直接跳过（构造条件检查）
            return;
        }
        String xml = prefix + "x".repeat(fill) + suffix;
        assertTrue(xml.getBytes(StandardCharsets.UTF_8).length < 8192,
            "测试 XML 应小于 8192 字节（前置条件）");

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertTrue(dispatch.handled(),
            "8192B 以内的合法 XML 应通过，实际=" + dispatch.handled()
                + " size=" + xml.getBytes(StandardCharsets.UTF_8).length);
    }

    @Test
    void xmlOver8192Bytes_rejected() {
        // 超过 8192 字节 → 必须拒绝
        String prefix = "<owo-ui><components><flow-layout><label id=\"a\">";
        String suffix = "</label></flow-layout></components></owo-ui>";
        int baseLen = prefix.getBytes(StandardCharsets.UTF_8).length
            + suffix.getBytes(StandardCharsets.UTF_8).length;
        // 构造确保超 8192
        String xml = prefix + "x".repeat(8192 - baseLen + 1) + suffix;

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xml));

        assertFalse(dispatch.handled(),
            "超过 8192 字节的 XML 应被拒绝，实际=" + dispatch.handled());
        assertTrue(dispatch.logMessage().contains("exceeds max supported"),
            "拒绝消息应含 'exceeds max supported'，实际=" + dispatch.logMessage());
    }

    // ─── 旧有限制仍然有效 ────────────────────────────────────────────────────

    @Test
    void doctypeXml_stillRejected() {
        String xmlWithDoctype = "<!DOCTYPE foo><owo-ui><components><flow-layout/></components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xmlWithDoctype));

        assertFalse(dispatch.handled(),
            "含 DOCTYPE 的 XML 仍应被拒绝，实际=" + dispatch.handled());
        assertTrue(dispatch.logMessage().contains("DOCTYPE"),
            "拒绝消息应含 'DOCTYPE'，实际=" + dispatch.logMessage());
    }

    @Test
    void scriptElement_stillRejected() {
        String xmlWithScript = "<owo-ui><components><flow-layout>"
            + "<script>alert(1)</script>"
            + "</flow-layout></components></owo-ui>";

        ServerDataDispatch dispatch = handler.handle(makeEnvelope(xmlWithScript));

        assertFalse(dispatch.handled(),
            "<script> 元素仍应被拒绝（非白名单），实际=" + dispatch.handled());
    }

    // ─── helper ──────────────────────────────────────────────────────────────

    /** 构造一个包含 xml 字段的 ui_open ServerDataEnvelope。 */
    private static ServerDataEnvelope makeEnvelope(String xml) {
        // JSON-escape xml string
        String escaped = xml
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t");
        String fullJson = "{\"v\":1,\"type\":\"ui_open\",\"screen_id\":\"agent_ui\",\"xml\":\"" + escaped + "\"}";
        var result = ServerDataEnvelope.parse(fullJson, fullJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(result.isSuccess(), "测试 envelope 构造失败：" + result.errorMessage());
        return result.envelope();
    }
}
