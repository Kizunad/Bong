package com.bong.client.fauna;

import org.junit.jupiter.api.Test;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import java.io.IOException;
import java.io.InputStream;
import java.util.HashSet;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-devour-rat-model P2 —— emissive 层**接线**核验（防功能孤岛）。
 *
 * <p>纯 JUnit 无法 bootstrap MC / GeckoLib 运行时，故 {@link FaunaRenderer} 与
 * {@link FaunaEmissiveGlowLayer} 都实例化不了。改用字节码核验这两条接线：
 * <ul>
 *   <li>{@code FaunaRenderer} 构造器真的 {@code addRenderLayer(new FaunaEmissiveGlowLayer(...))}
 *       ——只写了层类却没挂上，就是一个永不执行的孤岛，实机零发光；</li>
 *   <li>挂载受 {@code FaunaVisualKind.hasEmissiveGlow()} 门控——无条件挂会让没备
 *       {@code _glow} 资产的物种整只被 missing texture 紫黑格盖住；</li>
 *   <li>{@code FaunaEmissiveGlowLayer} 真的调 {@code FaunaModel.glowTextureFor} +
 *       {@code reRender}——不重绘就不会有第二层，发光同样不存在。</li>
 * </ul>
 */
class FaunaEmissiveGlowWiringTest {

    private record Bytecode(Set<String> invokedMethods, Set<String> newTypes) {
    }

    private static Bytecode scan(Class<?> target, String simpleName) {
        Set<String> invoked = new HashSet<>();
        Set<String> newTypes = new HashSet<>();
        try (InputStream input = target.getResourceAsStream(simpleName + ".class")) {
            if (input == null) {
                throw new AssertionError("读不到 " + simpleName + ".class，无法做接线字节码核验");
            }
            new ClassReader(input).accept(new ClassVisitor(Opcodes.ASM9) {
                @Override
                public MethodVisitor visitMethod(
                    int access, String name, String descriptor, String signature, String[] exceptions
                ) {
                    return new MethodVisitor(Opcodes.ASM9) {
                        @Override
                        public void visitMethodInsn(
                            int opcode, String owner, String methodName,
                            String methodDescriptor, boolean isInterface
                        ) {
                            invoked.add(methodName);
                        }

                        @Override
                        public void visitTypeInsn(int opcode, String type) {
                            if (opcode == Opcodes.NEW) {
                                newTypes.add(type);
                            }
                        }
                    };
                }
            }, ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
        } catch (IOException error) {
            throw new AssertionError("读取 " + simpleName + ".class 失败", error);
        }
        return new Bytecode(invoked, newTypes);
    }

    @Test
    void fauna_renderer_attaches_the_emissive_layer() {
        Bytecode renderer = scan(FaunaRenderer.class, "FaunaRenderer");

        assertTrue(
            renderer.newTypes().contains("com/bong/client/fauna/FaunaEmissiveGlowLayer"),
            "FaunaRenderer 必须 new FaunaEmissiveGlowLayer —— 只写层类不挂上 = 永不执行的孤岛，"
                + "实机零发光。实际 NEW 的类型: " + renderer.newTypes()
        );
        assertTrue(
            renderer.invokedMethods().contains("addRenderLayer"),
            "FaunaRenderer 必须调 addRenderLayer 把发光层挂进 GeoEntityRenderer，实际调用: "
                + renderer.invokedMethods()
        );
    }

    @Test
    void emissive_layer_attachment_is_gated_by_has_emissive_glow() {
        Bytecode renderer = scan(FaunaRenderer.class, "FaunaRenderer");
        assertTrue(
            renderer.invokedMethods().contains("hasEmissiveGlow"),
            "挂层必须受 FaunaVisualKind.hasEmissiveGlow() 门控 —— 无条件挂会让未备 _glow 资产的"
                + "物种整只被 missing texture 紫黑格盖住。实际调用: " + renderer.invokedMethods()
        );
    }

    @Test
    void emissive_layer_rerenders_model_with_derived_glow_texture() {
        Bytecode layer = scan(FaunaEmissiveGlowLayer.class, "FaunaEmissiveGlowLayer");

        assertTrue(
            layer.invokedMethods().contains("glowTextureFor"),
            "发光层必须调 FaunaModel.glowTextureFor 由**当前**底图推导 glow 贴图 —— 写死一张会让"
                + "噬元鼠换档（q0→q1→q2）时发光层不跟着换。实际调用: " + layer.invokedMethods()
        );
        assertTrue(
            layer.invokedMethods().contains("getTextureResource"),
            "发光层必须每帧取当前底图（getTextureResource），而不是构造期缓存。实际调用: "
                + layer.invokedMethods()
        );
        assertTrue(
            layer.invokedMethods().contains("reRender"),
            "发光层必须调 GeoRenderer.reRender 把模型用发光 RenderLayer 再画一遍 —— 不重绘就没有"
                + "第二层，发光不存在。实际调用: " + layer.invokedMethods()
        );
        assertTrue(
            layer.invokedMethods().contains("getEntityTranslucentEmissive"),
            "发光层必须走 RenderLayer.getEntityTranslucentEmissive（禁 lightmap = 全亮、alpha 生效 = "
                + "透明像素不画）。实际调用: " + layer.invokedMethods()
        );
    }

    @Test
    void full_bright_light_constant_is_max_lightmap() {
        // block=15, sky=15 → (15<<20) | (15<<4) = 15728640；与 GeckoLib AutoGlowingGeoLayer 同值。
        assertEquals(
            (15 << 20) | (15 << 4),
            FaunaEmissiveGlowLayer.FULL_BRIGHT_LIGHT,
            "全亮 packed light 必须是 lightmap 上限，否则发光层仍会被环境光压暗"
        );
    }
}
