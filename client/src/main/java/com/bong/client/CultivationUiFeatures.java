package com.bong.client;

import java.util.Objects;

public final class CultivationUiFeatures {
    static final boolean ENABLE_DYNAMIC_XML_UI = false;
    private static final String DYNAMIC_XML_UI_PAYLOAD_KIND = "dynamic_xml_ui";

    private CultivationUiFeatures() {
    }

    public static boolean isDynamicXmlUiEnabled() {
        return ENABLE_DYNAMIC_XML_UI;
    }

    static boolean shouldIgnoreServerDrivenUiPayload(String payloadKind) {
        Objects.requireNonNull(payloadKind, "payloadKind");
        return !ENABLE_DYNAMIC_XML_UI || !DYNAMIC_XML_UI_PAYLOAD_KIND.equals(payloadKind);
    }
}
