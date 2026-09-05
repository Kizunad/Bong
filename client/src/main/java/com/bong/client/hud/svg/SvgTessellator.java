package com.bong.client.hud.svg;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** 只负责把受限 SVG 图元转换为不可变三角形。 */
public final class SvgTessellator {
    public static final int MAX_TRIANGLES = 4096;
    private static final int ELLIPSE_SEGMENTS = 24;
    private static final double EPSILON = 1.0e-7;
    private static final double BOUNDS_EPSILON = 1.0e-4;

    public SvgMesh tessellate(SvgDocument document) {
        if (document == null) {
            throw new IllegalArgumentException("SVG document 不能为空");
        }
        List<SvgMesh.Triangle> triangles = new ArrayList<>();
        for (SvgDocument.Shape shape : document.shapes()) {
            List<SvgDocument.Point> normalizedPolygon = null;
            if (shape instanceof SvgDocument.Polygon polygon) {
                normalizedPolygon = validatePolygon(
                    polygon.points(),
                    document.width(),
                    document.height()
                );
            } else {
                validateShapeBounds(shape, document.width(), document.height());
            }
            int color = applyOpacity(shape.color(), shape.opacity());
            if ((color >>> 24) == 0) {
                continue;
            }
            if (shape instanceof SvgDocument.Rect rect) {
                addQuad(
                    triangles,
                    rect.x(),
                    rect.y(),
                    rect.x() + rect.width(),
                    rect.y() + rect.height(),
                    color
                );
            } else if (shape instanceof SvgDocument.Circle circle) {
                addEllipse(triangles, circle.cx(), circle.cy(), circle.radius(), circle.radius(), color);
            } else if (shape instanceof SvgDocument.Ellipse ellipse) {
                addEllipse(triangles, ellipse.cx(), ellipse.cy(), ellipse.rx(), ellipse.ry(), color);
            } else if (normalizedPolygon != null) {
                addNormalizedPolygon(triangles, normalizedPolygon, color);
            }
            if (triangles.size() > MAX_TRIANGLES) {
                throw new IllegalArgumentException("SVG 三角形数量超过预算: " + MAX_TRIANGLES);
            }
        }
        validateMeshBounds(triangles, document.width(), document.height());
        return new SvgMesh(document.width(), document.height(), triangles);
    }

    private static void addQuad(
        List<SvgMesh.Triangle> out,
        float x0,
        float y0,
        float x1,
        float y1,
        int color
    ) {
        SvgMesh.Vertex a = vertex(x0, y0, color);
        SvgMesh.Vertex b = vertex(x1, y0, color);
        SvgMesh.Vertex c = vertex(x1, y1, color);
        SvgMesh.Vertex d = vertex(x0, y1, color);
        addTriangle(out, a, b, c);
        addTriangle(out, a, c, d);
    }

    private static void addEllipse(
        List<SvgMesh.Triangle> out,
        float cx,
        float cy,
        float rx,
        float ry,
        int color
    ) {
        SvgMesh.Vertex center = vertex(cx, cy, color);
        for (int i = 0; i < ELLIPSE_SEGMENTS; i++) {
            double start = (Math.PI * 2.0 * i) / ELLIPSE_SEGMENTS;
            double end = (Math.PI * 2.0 * (i + 1)) / ELLIPSE_SEGMENTS;
            addTriangle(
                out,
                center,
                vertex(cx + (float) Math.cos(start) * rx, cy + (float) Math.sin(start) * ry, color),
                vertex(cx + (float) Math.cos(end) * rx, cy + (float) Math.sin(end) * ry, color)
            );
        }
    }

    /**
     * 使用耳切而不是 triangle fan；fan 只能正确覆盖凸多边形，凹 polygon 会产生
     * 越界三角形。输入先统一为逆时针（SVG 屏幕坐标中的正面积）并拒绝自交。
     */
    private static void addNormalizedPolygon(
        List<SvgMesh.Triangle> out,
        List<SvgDocument.Point> points,
        int color
    ) {
        double area = signedArea(points);
        if (Math.abs(area) <= EPSILON) {
            throw new IllegalArgumentException("SVG polygon 面积必须大于零");
        }
        if (area < 0.0) {
            Collections.reverse(points);
        }

        List<SvgDocument.Point> remaining = new ArrayList<>(points);
        int guard = 0;
        while (remaining.size() > 3) {
            boolean clipped = false;
            for (int i = 0; i < remaining.size(); i++) {
                int previousIndex = (i + remaining.size() - 1) % remaining.size();
                int nextIndex = (i + 1) % remaining.size();
                SvgDocument.Point previous = remaining.get(previousIndex);
                SvgDocument.Point current = remaining.get(i);
                SvgDocument.Point next = remaining.get(nextIndex);
                if (cross(previous, current, next) <= EPSILON) {
                    continue;
                }
                if (containsOtherPoint(remaining, previous, current, next, i)) {
                    continue;
                }
                addTriangle(
                    out,
                    vertex(previous.x(), previous.y(), color),
                    vertex(current.x(), current.y(), color),
                    vertex(next.x(), next.y(), color)
                );
                remaining.remove(i);
                clipped = true;
                break;
            }
            if (!clipped || ++guard > points.size() * points.size()) {
                throw new IllegalArgumentException("SVG polygon 无法进行有效耳切，可能存在自交或退化边");
            }
        }
        addTriangle(
            out,
            vertex(remaining.get(0).x(), remaining.get(0).y(), color),
            vertex(remaining.get(1).x(), remaining.get(1).y(), color),
            vertex(remaining.get(2).x(), remaining.get(2).y(), color)
        );
    }

    private static List<SvgDocument.Point> validatePolygon(
        List<SvgDocument.Point> input,
        float width,
        float height
    ) {
        List<SvgDocument.Point> points = normalizePolygon(input);
        if (Math.abs(signedArea(points)) <= EPSILON) {
            throw new IllegalArgumentException("SVG polygon 面积必须大于零");
        }
        for (SvgDocument.Point point : points) {
            requireBounds(point.x(), point.y(), width, height);
        }
        return points;
    }

    private static List<SvgDocument.Point> normalizePolygon(List<SvgDocument.Point> input) {
        if (input == null || input.size() < 3) {
            throw new IllegalArgumentException("SVG polygon 至少需要三个点");
        }
        List<SvgDocument.Point> points = new ArrayList<>(input.size());
        for (SvgDocument.Point point : input) {
            if (point == null) {
                throw new IllegalArgumentException("SVG polygon 不能包含 null 点");
            }
            if (points.isEmpty() || !samePoint(points.get(points.size() - 1), point)) {
                points.add(point);
            }
        }
        if (points.size() > 1 && samePoint(points.get(0), points.get(points.size() - 1))) {
            points.remove(points.size() - 1);
        }

        boolean changed;
        do {
            changed = false;
            if (points.size() < 3) {
                break;
            }
            for (int i = 0; i < points.size(); i++) {
                SvgDocument.Point previous = points.get((i + points.size() - 1) % points.size());
                SvgDocument.Point current = points.get(i);
                SvgDocument.Point next = points.get((i + 1) % points.size());
                if (Math.abs(cross(previous, current, next)) <= EPSILON
                    && between(previous, current, next)) {
                    points.remove(i);
                    changed = true;
                    break;
                }
            }
        } while (changed);

        if (points.size() < 3) {
            throw new IllegalArgumentException("SVG polygon 去重/去共线后少于三个点");
        }
        for (int i = 0; i < points.size(); i++) {
            for (int j = i + 1; j < points.size(); j++) {
                if (samePoint(points.get(i), points.get(j))) {
                    throw new IllegalArgumentException("SVG polygon 含非相邻重复点");
                }
            }
        }
        rejectSelfIntersections(points);
        return points;
    }

    private static void rejectSelfIntersections(List<SvgDocument.Point> points) {
        for (int i = 0; i < points.size(); i++) {
            SvgDocument.Point firstStart = points.get(i);
            SvgDocument.Point firstEnd = points.get((i + 1) % points.size());
            for (int j = i + 1; j < points.size(); j++) {
                if (i == j || areAdjacentEdges(i, j, points.size())) {
                    continue;
                }
                SvgDocument.Point secondStart = points.get(j);
                SvgDocument.Point secondEnd = points.get((j + 1) % points.size());
                if (segmentsIntersect(firstStart, firstEnd, secondStart, secondEnd)) {
                    throw new IllegalArgumentException("SVG polygon 含自交边");
                }
            }
        }
    }

    private static boolean areAdjacentEdges(int first, int second, int size) {
        return (first + 1) % size == second || (second + 1) % size == first;
    }

    private static boolean containsOtherPoint(
        List<SvgDocument.Point> points,
        SvgDocument.Point a,
        SvgDocument.Point b,
        SvgDocument.Point c,
        int ignoredIndex
    ) {
        for (int i = 0; i < points.size(); i++) {
            if (i == ignoredIndex
                || samePoint(points.get(i), a)
                || samePoint(points.get(i), b)
                || samePoint(points.get(i), c)) {
                continue;
            }
            if (pointInTriangle(points.get(i), a, b, c)) {
                return true;
            }
        }
        return false;
    }

    private static boolean pointInTriangle(
        SvgDocument.Point point,
        SvgDocument.Point a,
        SvgDocument.Point b,
        SvgDocument.Point c
    ) {
        double ab = cross(a, b, point);
        double bc = cross(b, c, point);
        double ca = cross(c, a, point);
        return ab >= -EPSILON && bc >= -EPSILON && ca >= -EPSILON;
    }

    private static boolean segmentsIntersect(
        SvgDocument.Point a,
        SvgDocument.Point b,
        SvgDocument.Point c,
        SvgDocument.Point d
    ) {
        double abC = cross(a, b, c);
        double abD = cross(a, b, d);
        double cdA = cross(c, d, a);
        double cdB = cross(c, d, b);
        if (Math.abs(abC) <= EPSILON && between(a, c, b)) return true;
        if (Math.abs(abD) <= EPSILON && between(a, d, b)) return true;
        if (Math.abs(cdA) <= EPSILON && between(c, a, d)) return true;
        if (Math.abs(cdB) <= EPSILON && between(c, b, d)) return true;
        return oppositeSigns(abC, abD) && oppositeSigns(cdA, cdB);
    }

    private static boolean oppositeSigns(double first, double second) {
        return (first > EPSILON && second < -EPSILON)
            || (first < -EPSILON && second > EPSILON);
    }

    private static boolean between(
        SvgDocument.Point a,
        SvgDocument.Point point,
        SvgDocument.Point b
    ) {
        return point.x() >= Math.min(a.x(), b.x()) - BOUNDS_EPSILON
            && point.x() <= Math.max(a.x(), b.x()) + BOUNDS_EPSILON
            && point.y() >= Math.min(a.y(), b.y()) - BOUNDS_EPSILON
            && point.y() <= Math.max(a.y(), b.y()) + BOUNDS_EPSILON;
    }

    private static double signedArea(List<SvgDocument.Point> points) {
        double area = 0.0;
        for (int i = 0; i < points.size(); i++) {
            SvgDocument.Point current = points.get(i);
            SvgDocument.Point next = points.get((i + 1) % points.size());
            area += (double) current.x() * next.y() - (double) next.x() * current.y();
        }
        return area * 0.5;
    }

    private static double cross(
        SvgDocument.Point a,
        SvgDocument.Point b,
        SvgDocument.Point c
    ) {
        return ((double) b.x() - a.x()) * (c.y() - a.y())
            - ((double) b.y() - a.y()) * (c.x() - a.x());
    }

    private static boolean samePoint(SvgDocument.Point first, SvgDocument.Point second) {
        return Math.abs(first.x() - second.x()) <= BOUNDS_EPSILON
            && Math.abs(first.y() - second.y()) <= BOUNDS_EPSILON;
    }

    private static void addTriangle(
        List<SvgMesh.Triangle> out,
        SvgMesh.Vertex a,
        SvgMesh.Vertex b,
        SvgMesh.Vertex c
    ) {
        if (out.size() >= MAX_TRIANGLES) {
            throw new IllegalArgumentException("SVG 三角形数量超过预算: " + MAX_TRIANGLES);
        }
        double area = ((double) b.x() - a.x()) * (c.y() - a.y())
            - ((double) b.y() - a.y()) * (c.x() - a.x());
        if (Math.abs(area) <= EPSILON) {
            throw new IllegalArgumentException("SVG 三角形面积必须大于零");
        }
        out.add(new SvgMesh.Triangle(a, b, c));
    }

    private static SvgMesh.Vertex vertex(float x, float y, int color) {
        return new SvgMesh.Vertex(x, y, color);
    }

    private static void validateShapeBounds(SvgDocument.Shape shape, float width, float height) {
        // 透明图元仍需验证几何，避免 malformed 资源借透明度绕过边界门。
        if (shape instanceof SvgDocument.Rect rect) {
            requireBounds(rect.x(), rect.y(), width, height);
            requireBounds(rect.x() + rect.width(), rect.y() + rect.height(), width, height);
        } else if (shape instanceof SvgDocument.Circle circle) {
            requireBounds(circle.cx() - circle.radius(), circle.cy() - circle.radius(), width, height);
            requireBounds(circle.cx() + circle.radius(), circle.cy() + circle.radius(), width, height);
        } else if (shape instanceof SvgDocument.Ellipse ellipse) {
            requireBounds(ellipse.cx() - ellipse.rx(), ellipse.cy() - ellipse.ry(), width, height);
            requireBounds(ellipse.cx() + ellipse.rx(), ellipse.cy() + ellipse.ry(), width, height);
        }
    }

    private static void validateMeshBounds(
        List<SvgMesh.Triangle> triangles,
        float width,
        float height
    ) {
        for (SvgMesh.Triangle triangle : triangles) {
            requireBounds(triangle.a().x(), triangle.a().y(), width, height);
            requireBounds(triangle.b().x(), triangle.b().y(), width, height);
            requireBounds(triangle.c().x(), triangle.c().y(), width, height);
        }
    }

    private static void requireBounds(float x, float y, float width, float height) {
        if (!Float.isFinite(x) || !Float.isFinite(y)
            || x < -BOUNDS_EPSILON
            || y < -BOUNDS_EPSILON
            || x > width + BOUNDS_EPSILON
            || y > height + BOUNDS_EPSILON) {
            throw new IllegalArgumentException(
                "SVG 几何超出 viewport: (" + x + "," + y + ") not in [0," + width + "]x[0," + height + "]"
            );
        }
    }

    static int applyOpacity(int color, float opacity) {
        int alpha = Math.round(((color >>> 24) & 0xFF) * opacity);
        return (alpha << 24) | (color & 0x00FFFFFF);
    }
}
