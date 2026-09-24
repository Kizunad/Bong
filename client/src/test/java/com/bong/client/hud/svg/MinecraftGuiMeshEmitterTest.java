package com.bong.client.hud.svg;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MinecraftGuiMeshEmitterTest {
    @Test
    void tintsArgbChannelsWithoutChangingGeometryContract() {
        assertEquals(0x40FF0020,
            MinecraftGuiMeshEmitter.tint(0x80FF8040, 0x80FF0080));
        assertEquals(0xFFFFFFFF,
            MinecraftGuiMeshEmitter.tint(0xFFFFFFFF, 0xFFFFFFFF));
        assertEquals(0x00000000,
            MinecraftGuiMeshEmitter.tint(0xFFFFFFFF, 0x00000000));
    }

    @Test
    void reversesSvgTriangleToMinecraftGuiFrontFaceAndClosesDegenerateQuad() {
        SvgMesh.Vertex a = new SvgMesh.Vertex(0.0f, 0.0f, 0xFFFFFFFF);
        SvgMesh.Vertex b = new SvgMesh.Vertex(10.0f, 0.0f, 0xFFFFFFFF);
        SvgMesh.Vertex c = new SvgMesh.Vertex(10.0f, 10.0f, 0xFFFFFFFF);
        SvgMesh.Triangle triangle = new SvgMesh.Triangle(a, b, c);

        SvgMesh.Vertex first = MinecraftGuiMeshEmitter.guiQuadVertex(triangle, 0);
        SvgMesh.Vertex second = MinecraftGuiMeshEmitter.guiQuadVertex(triangle, 1);
        SvgMesh.Vertex third = MinecraftGuiMeshEmitter.guiQuadVertex(triangle, 2);
        SvgMesh.Vertex fourth = MinecraftGuiMeshEmitter.guiQuadVertex(triangle, 3);

        assertEquals(a, first);
        assertEquals(c, second);
        assertEquals(b, third);
        assertEquals(third, fourth, "退化 quad 的末点必须重复，不能引入第二个可见三角形");
        assertTrue(signedArea(first, second, third) < 0.0f,
            "Minecraft GUI front face 要求与 DrawContext.fill 相同的负绕序");
        assertThrows(IllegalArgumentException.class,
            () -> MinecraftGuiMeshEmitter.guiQuadVertex(triangle, -1));
        assertThrows(IllegalArgumentException.class,
            () -> MinecraftGuiMeshEmitter.guiQuadVertex(triangle, 4));
    }

    private static float signedArea(SvgMesh.Vertex a, SvgMesh.Vertex b, SvgMesh.Vertex c) {
        return (b.x() - a.x()) * (c.y() - a.y())
            - (b.y() - a.y()) * (c.x() - a.x());
    }
}
