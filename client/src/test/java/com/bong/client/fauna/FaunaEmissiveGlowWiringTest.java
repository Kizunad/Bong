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

    /**
     * {@code invokedMethods} 收的是 <b>owner.name</b> 全限定形式（如
     * {@code com/bong/client/fauna/FaunaVisualKind.hasEmissiveGlow}）。
     *
     * <p>只收裸方法名判别力不够：任何 owner 上任何同名方法被调过就算通过，
     * 断言会退化成"某处调过一个叫 hasEmissiveGlow 的东西"。
     */
    private record Bytecode(Set<String> invokedMethods, Set<String> newTypes) {
        boolean invoked(String owner, String method) {
            return invokedMethods.contains(owner + "." + method);
        }
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
                            invoked.add(owner + "." + methodName);
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

    private static final String FAUNA_PKG = "com/bong/client/fauna/";

    @Test
    void fauna_renderer_attaches_the_emissive_layer() {
        Bytecode renderer = scan(FaunaRenderer.class, "FaunaRenderer");

        assertTrue(
            renderer.newTypes().contains("com/bong/client/fauna/FaunaEmissiveGlowLayer"),
            "FaunaRenderer 必须 new FaunaEmissiveGlowLayer —— 只写层类不挂上 = 永不执行的孤岛，"
                + "实机零发光。实际 NEW 的类型: " + renderer.newTypes()
        );
        // owner 是 FaunaRenderer 而非 GeoEntityRenderer：`addRenderLayer` 是继承来的方法、
        // 在 `this` 上调用，javac 按接收者静态类型（即本类）写 owner。
        assertTrue(
            renderer.invoked(FAUNA_PKG + "FaunaRenderer", "addRenderLayer"),
            "FaunaRenderer 必须调 addRenderLayer 把发光层挂进 GeoEntityRenderer，实际调用: "
                + renderer.invokedMethods()
        );
    }

    @Test
    void emissive_layer_attachment_is_gated_by_has_emissive_glow() {
        Bytecode renderer = scan(FaunaRenderer.class, "FaunaRenderer");
        assertTrue(
            renderer.invoked(FAUNA_PKG + "FaunaVisualKind", "hasEmissiveGlow"),
            "挂层必须受 **FaunaVisualKind**.hasEmissiveGlow() 门控 —— 无条件挂会让未备 _glow 资产的"
                + "物种整只被 missing texture 紫黑格盖住。实际调用: " + renderer.invokedMethods()
        );
        // 光有调用还不够：得证明它真的是**条件跳转**的判据，而不是一句被丢弃的无用调用
        // （例如塞进日志里）。hasEmissiveGlow 返回 boolean，门控必然伴随 IFEQ/IFNE 分支。
        assertTrue(
            hasConditionalBranchAfter(FaunaRenderer.class, "FaunaRenderer",
                FAUNA_PKG + "FaunaVisualKind", "hasEmissiveGlow"),
            "hasEmissiveGlow() 的返回值必须被条件跳转消费（门控挂层），而不是调完就丢——"
                + "后者等于无条件挂层，正好放过它声称要挡的失败"
        );
    }

    @Test
    void emissive_layer_rerenders_model_with_derived_glow_texture() {
        Bytecode layer = scan(FaunaEmissiveGlowLayer.class, "FaunaEmissiveGlowLayer");

        assertTrue(
            layer.invoked(FAUNA_PKG + "FaunaModel", "glowTextureFor"),
            "发光层必须调 **FaunaModel**.glowTextureFor 由当前底图推导 glow 贴图 —— 写死一张会让"
                + "噬元鼠换档（q0→q1→q2）时发光层不跟着换。实际调用: " + layer.invokedMethods()
        );
        assertTrue(
            layer.invoked(FAUNA_PKG + "FaunaEmissiveGlowLayer", "getTextureResource")
                || layer.invoked("software/bernie/geckolib/renderer/layer/GeoRenderLayer",
                    "getTextureResource"),
            "发光层必须每帧经 GeoRenderLayer.getTextureResource 取当前底图，而不是构造期缓存。"
                + "实际调用: " + layer.invokedMethods()
        );
        assertTrue(
            layer.invoked("software/bernie/geckolib/renderer/GeoRenderer", "reRender"),
            "发光层必须调 **GeoRenderer**.reRender 把模型用发光 RenderLayer 再画一遍 —— 不重绘就没有"
                + "第二层，发光不存在。实际调用: " + layer.invokedMethods()
        );
        assertTrue(
            layer.invoked("net/minecraft/client/render/RenderLayer", "getEntityTranslucentEmissive"),
            "发光层必须走 **RenderLayer**.getEntityTranslucentEmissive（禁 lightmap = 全亮、"
                + "alpha 生效 = 透明像素不画）。实际调用: " + layer.invokedMethods()
        );
    }

    /**
     * 扫描：目标类里是否存在「调用 {@code owner.method} 之后紧接着一条条件跳转」的指令序列。
     *
     * <p>用来区分"真门控"与"调完丢弃"——后者字节码里不会有消费返回值的条件跳转。
     */
    private static boolean hasConditionalBranchAfter(
        Class<?> target, String simpleName, String owner, String method
    ) {
        boolean[] found = {false};
        try (InputStream input = target.getResourceAsStream(simpleName + ".class")) {
            if (input == null) {
                throw new AssertionError("读不到 " + simpleName + ".class");
            }
            new ClassReader(input).accept(new ClassVisitor(Opcodes.ASM9) {
                @Override
                public MethodVisitor visitMethod(
                    int access, String name, String descriptor, String signature, String[] exceptions
                ) {
                    return new MethodVisitor(Opcodes.ASM9) {
                        private boolean armed = false;

                        @Override
                        public void visitMethodInsn(
                            int opcode, String insnOwner, String insnName,
                            String insnDescriptor, boolean isInterface
                        ) {
                            armed = owner.equals(insnOwner) && method.equals(insnName);
                        }

                        @Override
                        public void visitJumpInsn(int opcode, org.objectweb.asm.Label label) {
                            if (armed && (opcode == Opcodes.IFEQ || opcode == Opcodes.IFNE)) {
                                found[0] = true;
                            }
                            armed = false;
                        }
                    };
                }
            }, ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
        } catch (IOException error) {
            throw new AssertionError("读取 " + simpleName + ".class 失败", error);
        }
        return found[0];
    }

    @Test
    void full_bright_light_constant_is_max_lightmap() {
        // 用**字面量**断言，不复述生产里的 (15<<20)|(15<<4) 表达式——照抄表达式就成了同义反复，
        // 生产改成 (7<<20)|(7<<4) 也照样绿。15728880 = LightmapTextureManager.pack(15, 15)。
        assertEquals(
            15728880,
            FaunaEmissiveGlowLayer.FULL_BRIGHT_LIGHT,
            "全亮 packed light 必须是 lightmap 上限 15728880（block=15 & sky=15），"
                + "否则发光层在会读 lightmap 的 RenderLayer 下仍会被环境光压暗"
        );
    }
}
