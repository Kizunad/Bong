package com.bong.client.state;

import com.bong.client.BongClientFeatures;
import com.bong.client.network.ServerDataEnvelope;

import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.Objects;
import java.util.Optional;

public final class UiOpenState {
    private final Mode mode;
    private final String screenId;
    private final String templateId;
    private final String xmlLayout;

    private UiOpenState(Mode mode, String screenId, String templateId, String xmlLayout) {
        this.mode = Objects.requireNonNull(mode, "mode");
        this.screenId = Objects.requireNonNull(screenId, "screenId");
        this.templateId = templateId;
        this.xmlLayout = xmlLayout;
    }

    public static UiOpenState empty() {
        return new UiOpenState(Mode.NONE, "", null, null);
    }

    public static UiOpenState template(String screenId, String templateId) {
        return template(screenId, templateId, BongClientFeatures.ENABLE_XML_TEMPLATE_MODE);
    }

    public static UiOpenState template(String screenId, String templateId, boolean templateModeEnabled) {
        if (!templateModeEnabled) {
            return empty();
        }

        String normalizedScreenId = normalizeId(screenId);
        String normalizedTemplateId = normalizeId(templateId);
        if (normalizedScreenId.isEmpty() || normalizedTemplateId.isEmpty()) {
            return empty();
        }

        return new UiOpenState(Mode.TEMPLATE, normalizedScreenId, normalizedTemplateId, null);
    }

    public static UiOpenState dynamicXml(String screenId, String xmlLayout) {
        return dynamicXml(screenId, xmlLayout, BongClientFeatures.ENABLE_DYNAMIC_XML_UI);
    }

    public static UiOpenState dynamicXml(String screenId, String xmlLayout, boolean dynamicXmlEnabled) {
        if (!dynamicXmlEnabled) {
            return empty();
        }

        String normalizedScreenId = normalizeId(screenId);
        String normalizedXml = normalizeXml(xmlLayout);
        if (normalizedScreenId.isEmpty() || normalizedXml.isEmpty()) {
            return empty();
        }
        if (normalizedXml.getBytes(StandardCharsets.UTF_8).length > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return empty();
        }

        String normalizedXmlLowerCase = normalizedXml.toLowerCase(Locale.ROOT);
        if (normalizedXmlLowerCase.contains("<!doctype") || normalizedXmlLowerCase.contains("<!entity")) {
            return empty();
        }

        return new UiOpenState(Mode.RAW_XML, normalizedScreenId, null, normalizedXml);
    }

    private static String normalizeId(String value) {
        return value == null ? "" : value.trim();
    }

    private static String normalizeXml(String value) {
        return value == null ? "" : value.trim();
    }

    public Mode mode() {
        return mode;
    }

    public String screenId() {
        return screenId;
    }

    public Optional<String> templateId() {
        return Optional.ofNullable(templateId);
    }

    public Optional<String> xmlLayout() {
        return Optional.ofNullable(xmlLayout);
    }

    public boolean isEmpty() {
        return mode == Mode.NONE;
    }

    public boolean opensTemplate() {
        return mode == Mode.TEMPLATE;
    }

    public boolean opensDynamicXml() {
        return mode == Mode.RAW_XML;
    }

    public enum Mode {
        NONE,
        TEMPLATE,
        RAW_XML
    }
}
