package com.bong.client.hud.svg;

import javax.xml.XMLConstants;
import javax.xml.stream.XMLInputFactory;
import javax.xml.stream.XMLStreamConstants;
import javax.xml.stream.XMLStreamException;
import javax.xml.stream.XMLStreamReader;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * NanoSVG 边界的 Java adapter。
 *
 * <p>当前 vertical slice 使用 JDK 的安全 StAX 解析受限 SVG 子集，API 不暴露
 * parser/native 生命周期。后续替换为 JNI NanoSVG 时只需替换本类，不影响 mesh
 * 和 Minecraft GUI 提交层。</p>
 */
public final class NanoSvgParser implements SvgParser {
    static final int MAX_INPUT_BYTES = 1024 * 1024;
    private static final int MAX_SHAPES = 256;
    private static final int MAX_POINTS = 512;
    private static final int MAX_XML_EVENTS = 4096;
    private static final int MAX_XML_DEPTH = 8;
    private static final String SVG_NAMESPACE = "http://www.w3.org/2000/svg";
    private static final Pattern NUMBER = Pattern.compile(
        "[-+]?(?:\\d+(?:\\.\\d*)?|\\.\\d+)(?:[eE][-+]?\\d+)?(?:px)?",
        Pattern.CASE_INSENSITIVE
    );
    private static final Set<String> SVG_ATTRIBUTES = Set.of("viewBox", "width", "height");
    private static final Set<String> RECT_ATTRIBUTES = Set.of(
        "x", "y", "width", "height", "fill", "opacity", "fill-opacity"
    );
    private static final Set<String> CIRCLE_ATTRIBUTES = Set.of(
        "cx", "cy", "r", "fill", "opacity", "fill-opacity"
    );
    private static final Set<String> ELLIPSE_ATTRIBUTES = Set.of(
        "cx", "cy", "rx", "ry", "fill", "opacity", "fill-opacity"
    );
    private static final Set<String> POLYGON_ATTRIBUTES = Set.of(
        "points", "fill", "opacity", "fill-opacity"
    );
    private static final Set<String> GROUP_ATTRIBUTES = Set.of();

    @Override
    public SvgDocument parse(InputStream input) throws IOException {
        if (input == null) {
            throw new IllegalArgumentException("SVG 输入不能为空");
        }
        byte[] payload = readBounded(input);
        XMLInputFactory factory = XMLInputFactory.newFactory();
        setProperty(factory, XMLInputFactory.SUPPORT_DTD, false);
        setProperty(factory, XMLInputFactory.IS_SUPPORTING_EXTERNAL_ENTITIES, false);
        setProperty(factory, XMLInputFactory.IS_REPLACING_ENTITY_REFERENCES, false);
        setProperty(factory, XMLInputFactory.IS_NAMESPACE_AWARE, true);
        setProperty(factory, XMLConstants.ACCESS_EXTERNAL_DTD, "");
        setProperty(factory, XMLConstants.ACCESS_EXTERNAL_SCHEMA, "");
        factory.setXMLResolver((publicId, systemId, baseUri, namespace) -> {
            throw new XMLStreamException("SVG 禁止外部实体: " + systemId);
        });

        XMLStreamReader reader = null;
        try {
            reader = factory.createXMLStreamReader(new ByteArrayInputStream(payload));
            return parse(reader);
        } catch (XMLStreamException e) {
            throw new IOException("SVG XML 解析失败", e);
        } finally {
            closeReader(reader);
        }
    }

    private SvgDocument parse(XMLStreamReader reader) throws XMLStreamException {
        float width = -1.0f;
        float height = -1.0f;
        boolean rootSeen = false;
        boolean rootClosed = false;
        int depth = 0;
        int eventCount = 0;
        List<SvgDocument.Shape> shapes = new ArrayList<>();
        Deque<String> elements = new ArrayDeque<>();

        while (reader.hasNext()) {
            int event = reader.next();
            if (++eventCount > MAX_XML_EVENTS) {
                throw new IllegalArgumentException("SVG XML 事件数量超过预算: " + MAX_XML_EVENTS);
            }
            switch (event) {
                case XMLStreamConstants.START_ELEMENT -> {
                    depth++;
                    if (depth > MAX_XML_DEPTH) {
                        throw new IllegalArgumentException("SVG XML 嵌套深度超过预算: " + MAX_XML_DEPTH);
                    }
                    String name = reader.getLocalName();
                    requireSvgNamespace(reader, name);
                    if (!rootSeen) {
                        if (depth != 1 || !"svg".equals(name)) {
                            throw new IllegalArgumentException("SVG 根元素必须是 svg");
                        }
                        rejectAttributes(reader, SVG_ATTRIBUTES, "svg");
                        float[] viewBox = parseViewBox(reader.getAttributeValue(null, "viewBox"));
                        width = viewBox == null
                            ? parseLength(required(reader, "width", "svg"), "width")
                            : viewBox[2];
                        height = viewBox == null
                            ? parseLength(required(reader, "height", "svg"), "height")
                            : viewBox[3];
                        if (viewBox != null && (viewBox[0] != 0.0f || viewBox[1] != 0.0f)) {
                            throw new IllegalArgumentException(
                                "当前 SVG slice 只接受从 (0,0) 开始的 viewBox"
                            );
                        }
                        rootSeen = true;
                    } else {
                        if (rootClosed || depth == 1) {
                            throw new IllegalArgumentException("SVG 只允许一个根元素");
                        }
                        if (!elements.isEmpty() && isShapeElement(elements.peek())) {
                            throw new IllegalArgumentException("SVG 图元不能包含子元素: " + elements.peek());
                        }
                        switch (name) {
                            case "rect" -> shapes.add(parseRect(reader));
                            case "circle" -> shapes.add(parseCircle(reader));
                            case "ellipse" -> shapes.add(parseEllipse(reader));
                            case "polygon" -> shapes.add(parsePolygon(reader));
                            case "g" -> rejectAttributes(reader, GROUP_ATTRIBUTES, "g");
                            case "path", "line", "polyline", "text", "image", "use", "defs",
                                "clipPath", "mask", "filter", "linearGradient", "radialGradient",
                                "foreignObject" ->
                                throw new IllegalArgumentException("SVG 元素不在 V1 支持范围: " + name);
                            default -> throw new IllegalArgumentException("未知 SVG 元素: " + name);
                        }
                        if (shapes.size() > MAX_SHAPES) {
                            throw new IllegalArgumentException("SVG 图元数量超过预算: " + MAX_SHAPES);
                        }
                    }
                    elements.push(name);
                }
                case XMLStreamConstants.END_ELEMENT -> {
                    if (elements.isEmpty() || !elements.peek().equals(reader.getLocalName())) {
                        throw new IllegalArgumentException("SVG XML 元素嵌套不匹配");
                    }
                    String name = elements.pop();
                    if ("svg".equals(name)) {
                        if (depth != 1) {
                            throw new IllegalArgumentException("SVG 根元素层级错误");
                        }
                        rootClosed = true;
                    }
                    depth--;
                }
                case XMLStreamConstants.CHARACTERS, XMLStreamConstants.CDATA,
                    XMLStreamConstants.SPACE -> {
                    if (!reader.getText().isBlank()
                        && (elements.isEmpty() || isShapeElement(elements.peek()))) {
                        throw new IllegalArgumentException("SVG 图元只允许空白文本");
                    }
                }
                case XMLStreamConstants.DTD, XMLStreamConstants.ENTITY_REFERENCE,
                    XMLStreamConstants.ENTITY_DECLARATION, XMLStreamConstants.NOTATION_DECLARATION,
                    XMLStreamConstants.PROCESSING_INSTRUCTION ->
                    throw new IllegalArgumentException("SVG 禁止 DTD、entity 和处理指令");
                default -> {
                    // XML 声明和注释不改变受限 SVG 模型。
                }
            }
        }
        if (!rootSeen || !rootClosed || !elements.isEmpty() || depth != 0) {
            throw new IllegalArgumentException("SVG XML 缺少完整 svg 根元素");
        }
        if (width <= 0.0f || height <= 0.0f) {
            throw new IllegalArgumentException("SVG 缺少有效 viewport");
        }
        return new SvgDocument(width, height, shapes);
    }

    private SvgDocument.Rect parseRect(XMLStreamReader reader) {
        rejectAttributes(reader, RECT_ATTRIBUTES, "rect");
        return new SvgDocument.Rect(
            parseLength(required(reader, "x", "rect"), "rect.x"),
            parseLength(required(reader, "y", "rect"), "rect.y"),
            parseLength(required(reader, "width", "rect"), "rect.width"),
            parseLength(required(reader, "height", "rect"), "rect.height"),
            parseColor(reader.getAttributeValue(null, "fill")),
            parseOpacity(reader)
        );
    }

    private SvgDocument.Circle parseCircle(XMLStreamReader reader) {
        rejectAttributes(reader, CIRCLE_ATTRIBUTES, "circle");
        return new SvgDocument.Circle(
            parseLength(required(reader, "cx", "circle"), "circle.cx"),
            parseLength(required(reader, "cy", "circle"), "circle.cy"),
            parseLength(required(reader, "r", "circle"), "circle.r"),
            parseColor(reader.getAttributeValue(null, "fill")),
            parseOpacity(reader)
        );
    }

    private SvgDocument.Ellipse parseEllipse(XMLStreamReader reader) {
        rejectAttributes(reader, ELLIPSE_ATTRIBUTES, "ellipse");
        return new SvgDocument.Ellipse(
            parseLength(required(reader, "cx", "ellipse"), "ellipse.cx"),
            parseLength(required(reader, "cy", "ellipse"), "ellipse.cy"),
            parseLength(required(reader, "rx", "ellipse"), "ellipse.rx"),
            parseLength(required(reader, "ry", "ellipse"), "ellipse.ry"),
            parseColor(reader.getAttributeValue(null, "fill")),
            parseOpacity(reader)
        );
    }

    private SvgDocument.Polygon parsePolygon(XMLStreamReader reader) {
        rejectAttributes(reader, POLYGON_ATTRIBUTES, "polygon");
        String raw = required(reader, "points", "polygon");
        List<Float> values = parseNumberList(raw, "polygon.points", -1);
        if ((values.size() & 1) != 0) {
            throw new IllegalArgumentException("polygon.points 必须包含成对坐标");
        }
        if (values.size() / 2 > MAX_POINTS) {
            throw new IllegalArgumentException("SVG polygon 点数超过预算: " + MAX_POINTS);
        }
        List<SvgDocument.Point> points = new ArrayList<>(values.size() / 2);
        for (int i = 0; i < values.size(); i += 2) {
            points.add(new SvgDocument.Point(values.get(i), values.get(i + 1)));
        }
        return new SvgDocument.Polygon(
            points,
            parseColor(reader.getAttributeValue(null, "fill")),
            parseOpacity(reader)
        );
    }

    private static float[] parseViewBox(String raw) {
        if (raw == null || raw.isBlank()) {
            return null;
        }
        List<Float> values = parseNumberList(raw, "viewBox", 4);
        return new float[] {values.get(0), values.get(1), values.get(2), values.get(3)};
    }

    private static List<Float> parseNumberList(String raw, String name, int expected) {
        if (raw == null || raw.isBlank()) {
            throw new IllegalArgumentException(name + " 不能为空");
        }
        Matcher matcher = NUMBER.matcher(raw);
        List<Float> values = new ArrayList<>();
        int cursor = 0;
        while (matcher.find()) {
            String separator = raw.substring(cursor, matcher.start());
            String token = matcher.group();
            if (!isNumberSeparator(separator, values.isEmpty(), token)) {
                throw new IllegalArgumentException(name + " 包含无效分隔符: " + separator);
            }
            values.add(parseLength(token, name + "[" + values.size() + "]"));
            cursor = matcher.end();
        }
        if (!raw.substring(cursor).isBlank()) {
            throw new IllegalArgumentException(name + " 包含无效字符");
        }
        if (values.isEmpty() || (expected >= 0 && values.size() != expected)) {
            throw new IllegalArgumentException(
                name + " 必须包含 " + (expected >= 0 ? expected : "偶数个") + " 个数字"
            );
        }
        return values;
    }

    private static boolean isNumberSeparator(String separator, boolean first, String nextToken) {
        if (separator.isEmpty()) {
            // SVG 数字语法允许用正负号分隔相邻坐标，例如 "10-5"。
            if (first) {
                return !nextToken.isEmpty();
            }
            return !nextToken.isEmpty()
                && (nextToken.charAt(0) == '+' || nextToken.charAt(0) == '-');
        }
        boolean hasComma = false;
        int commaCount = 0;
        for (int i = 0; i < separator.length(); i++) {
            char ch = separator.charAt(i);
            if (ch == ',') {
                hasComma = true;
                commaCount++;
            } else if (!Character.isWhitespace(ch)) {
                return false;
            }
        }
        return commaCount <= 1 && (!first || !hasComma);
    }

    private static float parseLength(String raw, String name) {
        if (raw == null || raw.isBlank()) {
            throw new IllegalArgumentException(name + " 不能为空");
        }
        String value = raw.trim().toLowerCase(Locale.ROOT);
        if (value.endsWith("px")) {
            value = value.substring(0, value.length() - 2).trim();
        }
        try {
            float parsed = Float.parseFloat(value);
            if (!Float.isFinite(parsed)) {
                throw new IllegalArgumentException(name + " 必须是有限数");
            }
            return parsed;
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException(name + " 不是有效数字: " + raw, e);
        }
    }

    private static int parseColor(String raw) {
        if (raw == null || raw.isBlank()) {
            return 0xFFFFFFFF;
        }
        String value = raw.trim().toLowerCase(Locale.ROOT);
        if (value.equals("none")) {
            return 0;
        }
        if (value.equals("white")) return 0xFFFFFFFF;
        if (value.equals("black")) return 0xFF000000;
        if (value.equals("gray") || value.equals("grey")) return 0xFF808080;
        if (!value.startsWith("#")) {
            throw new IllegalArgumentException("不支持的 SVG fill: " + raw);
        }
        String hex = value.substring(1);
        if (hex.length() == 3) {
            hex = "" + hex.charAt(0) + hex.charAt(0)
                + hex.charAt(1) + hex.charAt(1)
                + hex.charAt(2) + hex.charAt(2);
        }
        if (hex.length() != 6 && hex.length() != 8) {
            throw new IllegalArgumentException("SVG fill 颜色格式错误: " + raw);
        }
        try {
            long parsed = Long.parseLong(hex, 16);
            return hex.length() == 6 ? (int) (0xFF000000L | parsed) : (int) parsed;
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException("SVG fill 颜色格式错误: " + raw, e);
        }
    }

    private static float parseOpacity(XMLStreamReader reader) {
        float opacity = parseOptionalUnit(reader.getAttributeValue(null, "opacity"), 1.0f, "opacity");
        float fillOpacity = parseOptionalUnit(
            reader.getAttributeValue(null, "fill-opacity"),
            1.0f,
            "fill-opacity"
        );
        return opacity * fillOpacity;
    }

    private static float parseOptionalUnit(String raw, float fallback, String name) {
        if (raw == null || raw.isBlank()) return fallback;
        float value = parseLength(raw, name);
        if (value < 0.0f || value > 1.0f) {
            throw new IllegalArgumentException(name + " 必须在 0 到 1 之间");
        }
        return value;
    }

    private static String required(XMLStreamReader reader, String name, String element) {
        String value = reader.getAttributeValue(null, name);
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException(element + " 缺少属性 " + name);
        }
        return value;
    }

    private static void requireSvgNamespace(XMLStreamReader reader, String element) {
        if (!SVG_NAMESPACE.equals(reader.getNamespaceURI())) {
            throw new IllegalArgumentException("SVG 元素 namespace 不受支持: " + element);
        }
    }

    private static void rejectAttributes(XMLStreamReader reader, Set<String> allowed, String element) {
        for (int i = 0; i < reader.getAttributeCount(); i++) {
            String namespace = reader.getAttributeNamespace(i);
            String name = reader.getAttributeLocalName(i);
            if (namespace != null && !namespace.isEmpty()) {
                throw new IllegalArgumentException("SVG 属性 namespace 不受支持: " + name);
            }
            if (!allowed.contains(name)) {
                throw new IllegalArgumentException(
                    "SVG " + element + " 属性不在 V1 支持范围: " + name
                );
            }
        }
    }

    private static boolean isShapeElement(String name) {
        return "rect".equals(name)
            || "circle".equals(name)
            || "ellipse".equals(name)
            || "polygon".equals(name);
    }

    private static byte[] readBounded(InputStream input) throws IOException {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int total = 0;
        int emptyReads = 0;
        while (true) {
            int count = input.read(buffer);
            if (count < 0) {
                break;
            }
            if (count == 0) {
                if (++emptyReads > 16) {
                    throw new IOException("SVG 输入流连续返回空读取");
                }
                continue;
            }
            emptyReads = 0;
            total += count;
            if (total > MAX_INPUT_BYTES) {
                throw new IOException("SVG 输入超过大小预算: " + MAX_INPUT_BYTES + " bytes");
            }
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }

    private static void closeReader(XMLStreamReader reader) {
        if (reader == null) {
            return;
        }
        try {
            reader.close();
        } catch (XMLStreamException ignored) {
            // 解析结果已经确定；关闭异常不能让 HUD 渲染线程崩溃。
        }
    }

    private static void setProperty(XMLInputFactory factory, String name, Object value) {
        try {
            factory.setProperty(name, value);
        } catch (IllegalArgumentException ignored) {
            // 不同 JDK 的 StAX 实现属性集合不同；事件级拒绝逻辑仍然生效。
        }
    }
}
