package com.bong.client.hud.svg;

import org.junit.jupiter.api.Test;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class NanoSvgParserTest {
    private final NanoSvgParser parser = new NanoSvgParser();

    @Test
    void parsesSupportedShapesAndPreservesOpacity() throws Exception {
        SvgDocument document = parser.parse(xml("""
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
              <rect x="1" y="2" width="10" height="8" fill="#123456" opacity="0.5"/>
              <circle cx="30" cy="20" r="4" fill="#fff"/>
              <polygon points="50,1 60,1 55,10" fill="#abcdef" fill-opacity="0.25"/>
            </svg>
            """));

        assertEquals(100.0f, document.width());
        assertEquals(50.0f, document.height());
        assertEquals(3, document.shapes().size());
        assertEquals(0x80123456, effectiveColor(document.shapes().get(0)));
        assertEquals(0x40ABCDEF, effectiveColor(document.shapes().get(2)));

        SvgMesh mesh = new SvgTessellator().tessellate(document);
        assertEquals(27, mesh.triangleCount(), "rect 2 + circle 24 + polygon 1");
    }

    @Test
    void convertsSvgRgbaColorsToMinecraftArgb() throws Exception {
        SvgDocument document = parser.parse(xml("""
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
              <rect x="0" y="0" width="10" height="10" fill="#11223380"/>
            </svg>
            """));

        assertEquals(
            0x80112233,
            effectiveColor(document.shapes().get(0)),
            "SVG #RRGGBBAA 必须重排为 Minecraft GUI 使用的 #AARRGGBB"
        );
    }

    @Test
    void acceptsMixedPolygonCoordinateSeparators() throws Exception {
        SvgDocument document = parser.parse(xml("""
            <svg xmlns="http://www.w3.org/2000/svg" width="40px" height="20px">
              <polygon points="0,0 10 0, 10,10 0 10" fill="#fff"/>
            </svg>
            """));

        SvgDocument.Polygon polygon = (SvgDocument.Polygon) document.shapes().get(0);
        assertEquals(40.0f, document.width());
        assertEquals(20.0f, document.height());
        assertEquals(List.of(
            new SvgDocument.Point(0.0f, 0.0f),
            new SvgDocument.Point(10.0f, 0.0f),
            new SvgDocument.Point(10.0f, 10.0f),
            new SvgDocument.Point(0.0f, 10.0f)
        ), polygon.points(), "polygon 坐标必须接受 SVG 允许的逗号和空白混合分隔");
    }

    @Test
    void rejectsUnsupportedElementsNamespacesAndAttributes() {
        assertThrows(IllegalArgumentException.class, () -> parser.parse(xml(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><path d=\"M0 0 L10 10\"/></svg>")));
        assertThrows(IllegalArgumentException.class, () -> parser.parse(xml(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\" data-owner=\"ui\"/>")));
        assertThrows(IllegalArgumentException.class, () -> parser.parse(xml(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:x=\"urn:test\" viewBox=\"0 0 10 10\"><rect x:x=\"0\" x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></svg>")));
        assertThrows(IllegalArgumentException.class, () -> parser.parse(xml(
            "<svg xmlns=\"urn:not-svg\" viewBox=\"0 0 10 10\"/>")));
        assertThrows(IllegalArgumentException.class, () -> parser.parse(xml(
            "<g xmlns=\"http://www.w3.org/2000/svg\"/>")));
    }

    @Test
    void rejectsDtdAndEntityPayloadsBeforeTheyBecomeGeometry() {
        assertRejected(
            "<!DOCTYPE svg [<!ENTITY x SYSTEM \"file:///etc/passwd\">]><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect x=\"&x;\" y=\"0\" width=\"1\" height=\"1\"/></svg>"
        );
        assertRejected(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect x=\"&unknown;\" y=\"0\" width=\"1\" height=\"1\"/></svg>"
        );
    }

    @Test
    void rejectsInputOverTheResourceBudget() {
        byte[] oversized = new byte[NanoSvgParser.MAX_INPUT_BYTES + 1];

        assertThrows(IOException.class, () -> parser.parse(new ByteArrayInputStream(oversized)));
    }

    @Test
    void rejectsMissingViewportAndInvalidColor() {
        assertThrows(IllegalArgumentException.class, () -> parser.parse(xml(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></svg>")));
        assertThrows(IllegalArgumentException.class, () -> parser.parse(xml(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\" fill=\"url(#paint)\"/></svg>")));
    }

    private static ByteArrayInputStream xml(String value) {
        return new ByteArrayInputStream(value.getBytes(StandardCharsets.UTF_8));
    }

    private void assertRejected(String value) {
        assertThrows(Exception.class, () -> parser.parse(xml(value)));
    }

    private static int effectiveColor(SvgDocument.Shape shape) {
        return SvgTessellator.applyOpacity(shape.color(), shape.opacity());
    }
}
