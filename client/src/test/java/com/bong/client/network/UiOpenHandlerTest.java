package com.bong.client.network;

import com.bong.client.state.UiOpenState;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class UiOpenHandlerTest {
    private final UiOpenHandler defaultHandler = new UiOpenHandler();

    @Test
    void validTemplatePayloadProducesRealUiOpenIntent() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-ui-open-template.json");

        ServerDataDispatch dispatch = defaultHandler.handle(parseEnvelope(json));
        UiOpenState uiOpenState = dispatch.uiOpenState().orElseThrow();

        assertTrue(dispatch.handled());
        assertTrue(uiOpenState.opensTemplate());
        assertEquals("cultivation_panel", uiOpenState.screenId());
        assertEquals("player_overview", uiOpenState.templateId().orElseThrow());
        assertTrue(uiOpenState.xmlLayout().isEmpty());
        assertTrue(dispatch.logMessage().contains("template 'player_overview'"));
    }

    @Test
    void templatePayloadAcceptsServerUiFieldAsScreenIdentifier() {
        ServerDataDispatch dispatch = defaultHandler.handle(parseEnvelope("""
            {"v":1,"type":"ui_open","ui":"cultivation_panel","template_id":"player_overview"}
            """));

        UiOpenState uiOpenState = dispatch.uiOpenState().orElseThrow();

        assertTrue(dispatch.handled());
        assertTrue(uiOpenState.opensTemplate());
        assertEquals("cultivation_panel", uiOpenState.screenId());
        assertEquals("player_overview", uiOpenState.templateId().orElseThrow());
        assertTrue(uiOpenState.xmlLayout().isEmpty());
    }

    @Test
    void templatePathWinsEvenWhenRawXmlIsAlsoPresent() {
        UiOpenHandler handler = new UiOpenHandler(true, true);

        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"ui_open","screen_id":"cultivation_panel","template_id":"player_overview",
             "xml":"<owo-ui><components><unknown-widget/></components></owo-ui>"}
            """));

        UiOpenState uiOpenState = dispatch.uiOpenState().orElseThrow();
        assertTrue(dispatch.handled());
        assertTrue(uiOpenState.opensTemplate());
        assertEquals("player_overview", uiOpenState.templateId().orElseThrow());
    }

    @Test
    void validRawXmlPayloadProducesDynamicUiOpenIntentWhenEnabled() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-ui-open-xml.json");

        ServerDataDispatch dispatch = new UiOpenHandler(true, true).handle(parseEnvelope(json));
        UiOpenState uiOpenState = dispatch.uiOpenState().orElseThrow();

        assertTrue(dispatch.handled());
        assertTrue(uiOpenState.opensDynamicXml());
        assertEquals("cultivation_panel", uiOpenState.screenId());
        assertTrue(uiOpenState.templateId().isEmpty());
        assertEquals(
            "<owo-ui><components><flow-layout><label/></flow-layout></components></owo-ui>",
            uiOpenState.xmlLayout().orElseThrow()
        );
        assertTrue(dispatch.logMessage().contains("guarded raw XML screen"));
    }

    @Test
    void rawXmlPayloadAcceptsServerUiFieldAsScreenIdentifier() {
        ServerDataDispatch dispatch = new UiOpenHandler(true, true).handle(parseEnvelope("""
            {"v":1,"type":"ui_open","ui":"cultivation_panel",
             "xml":"<owo-ui><components><flow-layout><label/></flow-layout></components></owo-ui>"}
            """));

        UiOpenState uiOpenState = dispatch.uiOpenState().orElseThrow();

        assertTrue(dispatch.handled());
        assertTrue(uiOpenState.opensDynamicXml());
        assertEquals("cultivation_panel", uiOpenState.screenId());
        assertTrue(uiOpenState.templateId().isEmpty());
        assertEquals(
            "<owo-ui><components><flow-layout><label/></flow-layout></components></owo-ui>",
            uiOpenState.xmlLayout().orElseThrow()
        );
    }

    @Test
    void rawXmlStaysBlockedWhenFeatureFlagIsDisabled() throws IOException {
        // plan-agent-ui-data-v1 P1: BongClientFeatures.ENABLE_DYNAMIC_XML_UI 已启用。
        // 使用显式 dynamicXmlEnabled=false 构造验证"关闭"分支仍拒绝 raw XML。
        String json = PayloadFixtureLoader.readText("valid-ui-open-xml-disabled.json");
        UiOpenHandler handlerWithXmlDisabled = new UiOpenHandler(true, false);

        ServerDataDispatch dispatch = handlerWithXmlDisabled.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "dynamicXmlEnabled=false 时 raw XML 应被拒绝（handled=false），实际=" + dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty(),
            "dynamicXmlEnabled=false 时 uiOpenState 应为空");
        assertTrue(dispatch.logMessage().contains("dynamic XML is disabled"),
            "拒绝消息应含 'dynamic XML is disabled'，实际=" + dispatch.logMessage());
    }

    @Test
    void rejectsDoctypeBeforeParsingIntoUiModel() throws IOException {
        ServerDataDispatch dispatch = new UiOpenHandler(true, true)
            .handle(parseEnvelope(PayloadFixtureLoader.readText("invalid-ui-open-doctype.json")));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty());
        assertTrue(dispatch.logMessage().contains("DOCTYPE"));
    }

    @Test
    void rejectsEntityBeforeParsingIntoUiModel() throws IOException {
        ServerDataDispatch dispatch = new UiOpenHandler(true, true)
            .handle(parseEnvelope(PayloadFixtureLoader.readText("invalid-ui-open-entity.json")));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty());
        assertTrue(dispatch.logMessage().contains("ENTITY"));
    }

    @Test
    void rejectsOversizeRawXmlBeforeParsingIntoUiModel() throws IOException {
        ServerDataDispatch dispatch = new UiOpenHandler(true, true)
            .handle(parseEnvelope(PayloadFixtureLoader.readText("invalid-ui-open-oversize-xml.json")));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty());
        assertTrue(dispatch.logMessage().contains("exceeds max supported size"));
    }

    @Test
    void rejectsOverlongRawXmlByStringLengthBeforeParsingIntoUiModel() throws IOException {
        // plan-agent-ui-data-v1 P1: MAX_XML_LAYOUT_BYTES = MAX_XML_LAYOUT_CHARACTERS = 8192。
        // 字节上限检查在字符长度检查前运行；超长 XML（>8192B）触发字节超限拒绝（"exceeds max supported size"）。
        // 两个检查都保护同一上限，字节检查先命中。
        ServerDataDispatch dispatch = new UiOpenHandler(true, true)
            .handle(parseEnvelope(PayloadFixtureLoader.readText("invalid-ui-open-overlong-xml.json")));

        assertFalse(dispatch.handled(),
            "超长 XML 应被拒绝（handled=false），实际=" + dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty(),
            "超长 XML 拒绝后 uiOpenState 应为空");
        assertTrue(dispatch.logMessage().contains("exceeds max supported"),
            "拒绝消息应含 'exceeds max supported'（字节或长度检查均命中），实际=" + dispatch.logMessage());
    }

    @Test
    void rejectsUnknownRootBeforeParsingIntoUiModel() throws IOException {
        ServerDataDispatch dispatch = new UiOpenHandler(true, true)
            .handle(parseEnvelope(PayloadFixtureLoader.readText("invalid-ui-open-unknown-root.json")));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty());
        assertTrue(dispatch.logMessage().contains("root must be <owo-ui>"));
    }

    @Test
    void rejectsUnknownComponentBeforeParsingIntoUiModel() throws IOException {
        ServerDataDispatch dispatch = new UiOpenHandler(true, true)
            .handle(parseEnvelope(PayloadFixtureLoader.readText("invalid-ui-open-unknown-component.json")));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty());
        assertTrue(dispatch.logMessage().contains("unknown component"));
    }

    @Test
    void rejectsDisallowedAttributeBeforeParsingIntoUiModel() throws IOException {
        ServerDataDispatch dispatch = new UiOpenHandler(true, true)
            .handle(parseEnvelope(PayloadFixtureLoader.readText("invalid-ui-open-disallowed-attribute.json")));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.uiOpenState().isEmpty());
        assertTrue(dispatch.logMessage().contains("disallowed attribute"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), () -> "Expected payload to parse successfully but got: " + parseResult.errorMessage());
        return parseResult.envelope();
    }
}
