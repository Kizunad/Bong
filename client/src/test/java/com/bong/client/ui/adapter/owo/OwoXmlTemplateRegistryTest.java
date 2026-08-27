package com.bong.client.ui.adapter.owo;

import io.wispforest.owo.ui.parsing.UIModel;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.InputStream;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class OwoXmlTemplateRegistryTest {
    @Test
    void productionRegistryContainsWideAndCompactCraftTemplates() {
        OwoXmlTemplateRegistry registry = OwoXmlTemplateRegistry.production();
        assertEquals(
            java.util.Set.of("craft", "craft-compact"),
            registry.templateIds(),
            "P2 reference slice 必须同时登记宽屏和最低 viewport 模板");
        assertEquals(OwoXmlTemplateRegistry.CRAFT, registry.identifierFor("craft"));
        assertEquals(OwoXmlTemplateRegistry.CRAFT_COMPACT, registry.identifierFor("craft-compact"));
    }

    @Test
    void checkedInCraftTemplatesAreValidOwoModels() throws Exception {
        for (String resource : new String[] {
            "/assets/bong/owo_ui/craft.xml",
            "/assets/bong/owo_ui/craft-compact.xml"
        }) {
            try (InputStream stream = getClass().getResourceAsStream(resource)) {
                assertNotNull(stream, "缺少随包发布的 owo XML: " + resource);
                assertNotNull(UIModel.load(stream), "owo 无法解析本地 XML: " + resource);
            }
        }
    }

    @Test
    void unknownBlankAndNullTemplateIdsAreRejected() {
        OwoXmlTemplateRegistry registry = new OwoXmlTemplateRegistry(ignored -> null, Map.of());
        assertThrows(IllegalArgumentException.class, () -> registry.require("unknown"));
        assertThrows(IllegalArgumentException.class, () -> registry.require("  "));
        assertThrows(NullPointerException.class, () -> registry.require(null));
    }

    @Test
    void registeredButMissingResourceFailsFast() {
        Identifier missing = new Identifier("bong", "missing");
        OwoXmlTemplateRegistry registry = new OwoXmlTemplateRegistry(ignored -> null, Map.of("missing", missing));
        IllegalStateException failure = assertThrows(IllegalStateException.class, () -> registry.require("missing"));
        assertTrue(failure.getMessage().contains("bong:missing"));
    }

    @Test
    void constructorRejectsMalformedRegistryEntries() {
        Identifier id = new Identifier("bong", "valid");
        assertThrows(NullPointerException.class, () -> new OwoXmlTemplateRegistry(null, Map.of()));
        assertThrows(NullPointerException.class, () -> new OwoXmlTemplateRegistry(ignored -> null, null));
        assertThrows(IllegalArgumentException.class, () ->
            new OwoXmlTemplateRegistry(ignored -> null, java.util.Collections.singletonMap(" " , id)));
        assertThrows(NullPointerException.class, () ->
            new OwoXmlTemplateRegistry(ignored -> null, java.util.Collections.singletonMap("valid", null)));
    }
}
