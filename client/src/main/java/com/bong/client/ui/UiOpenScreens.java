package com.bong.client.ui;

import com.bong.client.BongClient;
import com.bong.client.state.UiOpenState;
import io.wispforest.owo.ui.parsing.UIModel;
import net.minecraft.client.gui.screen.Screen;
import org.xml.sax.SAXException;

import javax.xml.parsers.ParserConfigurationException;
import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

public final class UiOpenScreens {
    public static final String CULTIVATION_PANEL_SCREEN_ID = "cultivation_panel";
    public static final String PLAYER_OVERVIEW_TEMPLATE_ID = "player_overview";

    private UiOpenScreens() {
    }

    public static boolean isRegisteredTemplate(String screenId, String templateId) {
        return CULTIVATION_PANEL_SCREEN_ID.equals(normalizeId(screenId))
            && PLAYER_OVERVIEW_TEMPLATE_ID.equals(normalizeId(templateId));
    }

    public static Screen createScreen(UiOpenState uiOpenState) {
        if (uiOpenState == null || uiOpenState.isEmpty()) {
            return null;
        }
        if (uiOpenState.opensTemplate()) {
            return createTemplateScreen(uiOpenState);
        }
        if (uiOpenState.opensDynamicXml()) {
            return createDynamicXmlScreen(uiOpenState);
        }
        return null;
    }

    private static Screen createTemplateScreen(UiOpenState uiOpenState) {
        String templateId = uiOpenState.templateId().orElse("");
        if (isRegisteredTemplate(uiOpenState.screenId(), templateId)) {
            return CultivationScreenBootstrap.createScreenForCurrentState();
        }

        BongClient.LOGGER.warn(
            "No registered client template for ui_open screen '{}' and template '{}'",
            uiOpenState.screenId(),
            templateId
        );
        return null;
    }

    private static Screen createDynamicXmlScreen(UiOpenState uiOpenState) {
        String xmlLayout = uiOpenState.xmlLayout().orElse("");
        try (ByteArrayInputStream inputStream = new ByteArrayInputStream(xmlLayout.getBytes(StandardCharsets.UTF_8))) {
            UIModel model = UIModel.load(inputStream);
            return new DynamicXmlScreen(uiOpenState.screenId(), model);
        } catch (ParserConfigurationException | IOException | SAXException | RuntimeException exception) {
            BongClient.LOGGER.error("Failed to create raw XML ui_open screen for '{}'", uiOpenState.screenId(), exception);
            return null;
        }
    }

    private static String normalizeId(String value) {
        return value == null ? "" : value.trim();
    }
}
