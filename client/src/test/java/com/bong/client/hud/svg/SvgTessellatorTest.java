package com.bong.client.hud.svg;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SvgTessellatorTest {
    @Test
    void convertsRectCircleAndPolygonToImmutableMesh() {
        SvgDocument document = new SvgDocument(64, 64, List.of(
            new SvgDocument.Rect(0, 0, 10, 20, 0xFFFFFFFF, 1.0f),
            new SvgDocument.Circle(20, 20, 4, 0x80FF0000, 1.0f),
            new SvgDocument.Polygon(List.of(
                new SvgDocument.Point(30, 0),
                new SvgDocument.Point(40, 0),
                new SvgDocument.Point(35, 10)
            ), 0xFF00FF00, 1.0f)
        ));

        SvgMesh mesh = new SvgTessellator().tessellate(document);

        assertEquals(27, mesh.triangleCount());
        assertEquals(81, mesh.vertexCount());
        assertEquals(0x80FF0000, mesh.triangles().get(2).a().color());
        assertThrows(UnsupportedOperationException.class, () -> mesh.triangles().clear());
    }

    @Test
    void skipsFullyTransparentShapesAndRejectsNonFiniteGeometry() {
        SvgDocument transparent = new SvgDocument(10, 10, List.of(
            new SvgDocument.Rect(0, 0, 1, 1, 0xFFFFFFFF, 0.0f)
        ));
        assertTrue(new SvgTessellator().tessellate(transparent).triangles().isEmpty());
        assertThrows(IllegalArgumentException.class,
            () -> new SvgDocument.Circle(Float.NaN, 0, 1, 0xFFFFFFFF, 1.0f));
    }

    @Test
    void triangulatesConcavePolygonsWithoutFillingTheConcavity() {
        SvgDocument document = new SvgDocument(10, 10, List.of(
            new SvgDocument.Polygon(List.of(
                new SvgDocument.Point(0, 0),
                new SvgDocument.Point(6, 0),
                new SvgDocument.Point(6, 6),
                new SvgDocument.Point(3, 3),
                new SvgDocument.Point(0, 6)
            ), 0xFFFFFFFF, 1.0f)
        ));

        SvgMesh mesh = new SvgTessellator().tessellate(document);

        assertEquals(3, mesh.triangleCount());
        assertEquals(27.0d, meshArea(mesh), 0.0001d,
            "耳切后三角形总面积必须等于凹 polygon 面积，不能用 triangle fan 填平凹口");
    }

    @Test
    void rejectsDegenerateSelfIntersectingAndOutOfViewportPolygonsEvenWhenTransparent() {
        SvgTessellator tessellator = new SvgTessellator();

        assertThrows(IllegalArgumentException.class, () -> tessellator.tessellate(new SvgDocument(10, 10, List.of(
            new SvgDocument.Polygon(List.of(
                new SvgDocument.Point(0, 0),
                new SvgDocument.Point(10, 10),
                new SvgDocument.Point(0, 10),
                new SvgDocument.Point(10, 0)
            ), 0xFFFFFFFF, 1.0f)
        ))), "自交 polygon 不能进入 GUI mesh");
        assertThrows(IllegalArgumentException.class, () -> tessellator.tessellate(new SvgDocument(10, 10, List.of(
            new SvgDocument.Polygon(List.of(
                new SvgDocument.Point(0, 0),
                new SvgDocument.Point(5, 0),
                new SvgDocument.Point(10, 0)
            ), 0xFFFFFFFF, 1.0f)
        ))), "零面积 polygon 不能进入 GUI mesh");
        assertThrows(IllegalArgumentException.class, () -> tessellator.tessellate(new SvgDocument(10, 10, List.of(
            new SvgDocument.Polygon(List.of(
                new SvgDocument.Point(0, 0),
                new SvgDocument.Point(11, 0),
                new SvgDocument.Point(0, 10)
            ), 0xFFFFFFFF, 0.0f)
        ))), "透明 polygon 也必须经过 viewport 校验，不能借 opacity 绕过资源门");
    }

    @Test
    void normalizesClosingPointAndRejectsOtherViewportOverflow() {
        SvgTessellator tessellator = new SvgTessellator();
        SvgDocument closed = new SvgDocument(10, 10, List.of(
            new SvgDocument.Polygon(List.of(
                new SvgDocument.Point(0, 0),
                new SvgDocument.Point(10, 0),
                new SvgDocument.Point(10, 10),
                new SvgDocument.Point(0, 10),
                new SvgDocument.Point(0, 0)
            ), 0xFFFFFFFF, 1.0f)
        ));

        assertEquals(2, tessellator.tessellate(closed).triangleCount(),
            "与首点重复的闭合点应在三角化前归一化");
        assertThrows(IllegalArgumentException.class, () -> tessellator.tessellate(new SvgDocument(10, 10, List.of(
            new SvgDocument.Rect(0, 0, 11, 1, 0xFFFFFFFF, 1.0f)
        ))), "非 polygon 图元同样不能越过 viewport");
    }

    private static double meshArea(SvgMesh mesh) {
        return mesh.triangles().stream()
            .mapToDouble(triangle -> Math.abs(
                ((double) triangle.b().x() - triangle.a().x())
                    * (triangle.c().y() - triangle.a().y())
                    - ((double) triangle.b().y() - triangle.a().y())
                        * (triangle.c().x() - triangle.a().x())
            ) * 0.5d)
            .sum();
    }
}
