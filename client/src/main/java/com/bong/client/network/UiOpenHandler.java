package com.bong.client.network;

import com.bong.client.BongClientFeatures;
import com.bong.client.state.UiOpenState;
import com.bong.client.ui.UiOpenScreens;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.xml.sax.InputSource;
import org.xml.sax.SAXException;

import javax.xml.XMLConstants;
import javax.xml.parsers.DocumentBuilder;
import javax.xml.parsers.DocumentBuilderFactory;
import javax.xml.parsers.ParserConfigurationException;
import java.io.IOException;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.function.BiPredicate;

public final class UiOpenHandler implements ServerDataHandler {
    static final int MAX_XML_LAYOUT_BYTES = 512;
    static final int MAX_XML_LAYOUT_CHARACTERS = 384;

    private static final String ROOT_ELEMENT = "owo-ui";
    private static final String COMPONENTS_ELEMENT = "components";
    private static final Set<String> ALLOWED_COMPONENT_ELEMENTS = Set.of("flow-layout", "label");
    private static final Map<String, Set<String>> ALLOWED_ATTRIBUTES_BY_ELEMENT = Map.of(
        ROOT_ELEMENT, Set.of(),
        COMPONENTS_ELEMENT, Set.of(),
        "flow-layout", Set.of(),
        "label", Set.of()
    );

    private final boolean templateModeEnabled;
    private final boolean dynamicXmlEnabled;
    private final BiPredicate<String, String> supportedTemplateMatcher;

    public UiOpenHandler() {
        this(
            BongClientFeatures.ENABLE_XML_TEMPLATE_MODE,
            BongClientFeatures.ENABLE_DYNAMIC_XML_UI,
            UiOpenScreens::isRegisteredTemplate
        );
    }

    UiOpenHandler(boolean templateModeEnabled, boolean dynamicXmlEnabled) {
        this(templateModeEnabled, dynamicXmlEnabled, UiOpenScreens::isRegisteredTemplate);
    }

    UiOpenHandler(
        boolean templateModeEnabled,
        boolean dynamicXmlEnabled,
        BiPredicate<String, String> supportedTemplateMatcher
    ) {
        this.templateModeEnabled = templateModeEnabled;
        this.dynamicXmlEnabled = dynamicXmlEnabled;
        this.supportedTemplateMatcher = Objects.requireNonNull(supportedTemplateMatcher, "supportedTemplateMatcher");
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String screenId = firstNonBlank(readOptionalString(payload, "ui"), readOptionalString(payload, "screen_id"));
        String templateId = readOptionalString(payload, "template_id");
        String xmlLayout = firstNonBlank(readOptionalString(payload, "xml"), readOptionalString(payload, "xml_layout"));

        Resolution templateResolution = resolveTemplateOpenState(screenId, templateId);
        if (templateResolution.state() != null) {
            UiOpenState uiOpenState = templateResolution.state();
            return ServerDataDispatch.handledWithUiOpen(
                envelope.type(),
                uiOpenState,
                "Routed ui_open payload to template '"
                    + uiOpenState.templateId().orElseThrow()
                    + "' for screen '"
                    + uiOpenState.screenId()
                    + "'"
            );
        }

        Resolution xmlResolution = resolveDynamicXmlOpenState(screenId, xmlLayout);
        if (xmlResolution.state() != null) {
            UiOpenState uiOpenState = xmlResolution.state();
            return ServerDataDispatch.handledWithUiOpen(
                envelope.type(),
                uiOpenState,
                "Routed ui_open payload to guarded raw XML screen '" + uiOpenState.screenId() + "'"
            );
        }

        List<String> reasons = new ArrayList<>();
        if (!templateResolution.reason().isBlank()) {
            reasons.add(templateResolution.reason());
        }
        if (!xmlResolution.reason().isBlank()) {
            reasons.add(xmlResolution.reason());
        }
        if (reasons.isEmpty()) {
            reasons.add("neither a supported template_id nor a raw XML payload was provided");
        }

        return ServerDataDispatch.noOp(
            envelope.type(),
            "Ignoring ui_open payload because " + String.join("; ", reasons)
        );
    }

    private Resolution resolveTemplateOpenState(String screenId, String templateId) {
        String normalizedTemplateId = normalizeText(templateId);
        if (normalizedTemplateId.isEmpty()) {
            return Resolution.failure("");
        }

        UiOpenState uiOpenState = UiOpenState.template(screenId, normalizedTemplateId, templateModeEnabled);
        if (uiOpenState.isEmpty()) {
            if (!templateModeEnabled) {
                return Resolution.failure("template-driven ui_open is disabled");
            }
            return Resolution.failure("template_id requires a non-blank ui or screen_id plus template_id");
        }

        if (!supportedTemplateMatcher.test(uiOpenState.screenId(), uiOpenState.templateId().orElseThrow())) {
            return Resolution.failure(
                "template '"
                    + uiOpenState.templateId().orElseThrow()
                    + "' is not registered for screen '"
                    + uiOpenState.screenId()
                    + "'"
            );
        }

        return Resolution.success(uiOpenState);
    }

    private Resolution resolveDynamicXmlOpenState(String screenId, String xmlLayout) {
        String normalizedXml = normalizeText(xmlLayout);
        if (normalizedXml.isEmpty()) {
            return Resolution.failure("");
        }
        if (!dynamicXmlEnabled) {
            return Resolution.failure("dynamic XML is disabled");
        }

        int xmlSizeBytes = normalizedXml.getBytes(StandardCharsets.UTF_8).length;
        if (xmlSizeBytes > MAX_XML_LAYOUT_BYTES) {
            return Resolution.failure(
                "raw XML exceeds max supported size of " + MAX_XML_LAYOUT_BYTES + " bytes: " + xmlSizeBytes
            );
        }

        int xmlLengthCharacters = normalizedXml.length();
        if (xmlLengthCharacters > MAX_XML_LAYOUT_CHARACTERS) {
            return Resolution.failure(
                "raw XML exceeds max supported length of "
                    + MAX_XML_LAYOUT_CHARACTERS
                    + " characters: "
                    + xmlLengthCharacters
            );
        }

        String lowerCaseXml = normalizedXml.toLowerCase(Locale.ROOT);
        if (lowerCaseXml.contains("<!doctype")) {
            return Resolution.failure("raw XML contains forbidden DOCTYPE declaration");
        }
        if (lowerCaseXml.contains("<!entity")) {
            return Resolution.failure("raw XML contains forbidden ENTITY declaration");
        }

        Validation validation = validateXmlDocument(normalizedXml);
        if (!validation.valid()) {
            return Resolution.failure(validation.reason());
        }

        UiOpenState uiOpenState = UiOpenState.dynamicXml(screenId, normalizedXml, true);
        if (uiOpenState.isEmpty()) {
            return Resolution.failure("raw XML payload failed UiOpenState guards");
        }

        return Resolution.success(uiOpenState);
    }

    private static Validation validateXmlDocument(String xmlLayout) {
        try {
            DocumentBuilder builder = createSecureDocumentBuilder();
            Document document = builder.parse(new InputSource(new StringReader(xmlLayout)));
            Element rootElement = document.getDocumentElement();
            if (rootElement == null) {
                return Validation.failure("raw XML document was empty");
            }
            if (!ROOT_ELEMENT.equals(rootElement.getNodeName())) {
                return Validation.failure("raw XML root must be <" + ROOT_ELEMENT + "> but was <" + rootElement.getNodeName() + ">"
                );
            }

            Validation rootAttributesValidation = validateAllowedAttributes(rootElement);
            if (!rootAttributesValidation.valid()) {
                return rootAttributesValidation;
            }

            Element componentsElement = null;
            for (Element child : childElements(rootElement)) {
                if (!COMPONENTS_ELEMENT.equals(child.getNodeName())) {
                    return Validation.failure("raw XML contains unknown root child <" + child.getNodeName() + ">"
                    );
                }

                Validation childAttributesValidation = validateAllowedAttributes(child);
                if (!childAttributesValidation.valid()) {
                    return childAttributesValidation;
                }

                if (componentsElement != null) {
                    return Validation.failure("raw XML must declare exactly one <components> section");
                }
                componentsElement = child;
            }

            if (componentsElement == null) {
                return Validation.failure("raw XML is missing required <components> section");
            }

            List<Element> componentRoots = childElements(componentsElement);
            if (componentRoots.size() != 1) {
                return Validation.failure("raw XML <components> section must contain exactly one root component");
            }

            return validateComponentTree(componentRoots.get(0));
        } catch (ParserConfigurationException exception) {
            return Validation.failure("raw XML secure parser configuration failed: " + exception.getMessage());
        } catch (SAXException | IOException exception) {
            return Validation.failure("raw XML could not be parsed safely: " + exception.getMessage());
        }
    }

    private static Validation validateComponentTree(Element componentElement) {
        if (!ALLOWED_COMPONENT_ELEMENTS.contains(componentElement.getNodeName())) {
            return Validation.failure("raw XML contains unknown component <" + componentElement.getNodeName() + ">"
            );
        }

        Validation attributesValidation = validateAllowedAttributes(componentElement);
        if (!attributesValidation.valid()) {
            return attributesValidation;
        }

        for (Element child : childElements(componentElement)) {
            Validation childValidation = validateComponentTree(child);
            if (!childValidation.valid()) {
                return childValidation;
            }
        }

        return Validation.success();
    }

    private static Validation validateAllowedAttributes(Element element) {
        Set<String> allowedAttributes = ALLOWED_ATTRIBUTES_BY_ELEMENT.get(element.getNodeName());
        if (allowedAttributes == null) {
            return Validation.failure("raw XML contains unknown element <" + element.getNodeName() + ">"
            );
        }

        for (int index = 0; index < element.getAttributes().getLength(); index++) {
            Node attribute = element.getAttributes().item(index);
            if (!allowedAttributes.contains(attribute.getNodeName())) {
                return Validation.failure(
                    "raw XML element <"
                        + element.getNodeName()
                        + "> contains disallowed attribute '"
                        + attribute.getNodeName()
                        + "'"
                );
            }
        }

        return Validation.success();
    }

    private static DocumentBuilder createSecureDocumentBuilder() throws ParserConfigurationException {
        DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
        factory.setNamespaceAware(false);
        factory.setXIncludeAware(false);
        factory.setExpandEntityReferences(false);
        factory.setFeature(XMLConstants.FEATURE_SECURE_PROCESSING, true);
        factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
        factory.setFeature("http://xml.org/sax/features/external-general-entities", false);
        factory.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
        factory.setFeature("http://apache.org/xml/features/nonvalidating/load-external-dtd", false);
        return factory.newDocumentBuilder();
    }

    private static List<Element> childElements(Element element) {
        List<Element> children = new ArrayList<>();
        for (int index = 0; index < element.getChildNodes().getLength(); index++) {
            Node child = element.getChildNodes().item(index);
            if (child.getNodeType() == Node.ELEMENT_NODE) {
                children.add((Element) child);
            }
        }
        return children;
    }

    private static String firstNonBlank(String... candidates) {
        if (candidates == null) {
            return null;
        }

        for (String candidate : candidates) {
            String normalizedCandidate = normalizeText(candidate);
            if (!normalizedCandidate.isEmpty()) {
                return normalizedCandidate;
            }
        }

        return null;
    }

    private static String normalizeText(String value) {
        return value == null ? "" : value.trim();
    }

    private static String readOptionalString(JsonObject payload, String fieldName) {
        JsonPrimitive primitive = readPrimitive(payload, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static JsonPrimitive readPrimitive(JsonObject payload, String fieldName) {
        JsonElement element = payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }

    private record Resolution(UiOpenState state, String reason) {
        private static Resolution success(UiOpenState state) {
            return new Resolution(state, "");
        }

        private static Resolution failure(String reason) {
            return new Resolution(null, reason == null ? "" : reason);
        }
    }

    private record Validation(boolean valid, String reason) {
        private static Validation success() {
            return new Validation(true, "");
        }

        private static Validation failure(String reason) {
            return new Validation(false, reason == null ? "raw XML validation failed" : reason);
        }
    }
}
