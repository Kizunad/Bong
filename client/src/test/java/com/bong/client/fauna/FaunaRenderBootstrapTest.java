package com.bong.client.fauna;

import net.minecraft.util.Identifier;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class FaunaRenderBootstrapTest {
    @Test
    void faunaVisualKindsPinEntityRawIdOrderAfterWhale() {
        assertFaunaRawId(FaunaVisualKind.DEVOUR_RAT, 126);
        assertFaunaRawId(FaunaVisualKind.ASH_SPIDER, 127);
        assertFaunaRawId(FaunaVisualKind.HYBRID_BEAST, 128);
        assertFaunaRawId(FaunaVisualKind.VOID_DISTORTED, 129);
        assertFaunaRawId(FaunaVisualKind.DAOXIANG, 130);
        assertFaunaRawId(FaunaVisualKind.ZHINIAN, 131);
        assertFaunaRawId(FaunaVisualKind.TSY_SENTINEL, 132);
        assertFaunaRawId(FaunaVisualKind.FUYA, 133);
        assertFaunaRawId(FaunaVisualKind.SKULL_FIEND, 134);
    }

    @Test
    void allPlannedNonWhaleFaunaModelsHaveStableResourcePaths() {
        Set<String> paths = Arrays.stream(FaunaVisualKind.values())
            .map(kind -> kind.modelId().getPath())
            .collect(Collectors.toSet());

        Set<String> expected = Set.of(
            "geo/devour_rat.geo.json",
            "geo/ash_spider.geo.json",
            "geo/hybrid_beast.geo.json",
            "geo/void_distorted.geo.json",
            "geo/daoxiang.geo.json",
            "geo/zhinian.geo.json",
            "geo/tsy_sentinel.geo.json",
            "geo/fuya.geo.json",
            "geo/skull_fiend.geo.json"
        );
        assertEquals(
            expected,
            paths,
            "expected all planned fauna model resource paths because server/client raw-id mapping depends on stable fauna assets, actual: "
                + paths
        );
    }

    @Test
    void fuyaTextureUsesEntityFaunaNamespace() {
        Identifier texture = FaunaVisualKind.FUYA.textureId();
        assertEquals(
            "bong",
            texture.getNamespace(),
            "expected FUYA texture namespace to be bong because fauna textures are bundled under Bong assets, actual: "
                + texture
        );
        assertEquals(
            "textures/entity/fauna/fuya.png",
            texture.getPath(),
            "expected FUYA texture path under textures/entity/fauna because renderer lookup uses fauna asset layout, actual: "
                + texture
        );
    }

    @Test
    void skullFiendTextureUsesEntityFaunaNamespace() {
        Identifier texture = FaunaVisualKind.SKULL_FIEND.textureId();
        assertEquals(
            "bong",
            texture.getNamespace(),
            "expected SKULL_FIEND texture namespace to be bong because fauna textures are bundled under Bong assets, actual: "
                + texture
        );
        assertEquals(
            "textures/entity/fauna/skull_fiend.png",
            texture.getPath(),
            "expected SKULL_FIEND texture path under textures/entity/fauna because renderer lookup uses fauna asset layout, actual: "
                + texture
        );
    }

    @Test
    void faunaEntityExplicitlyParticipatesInCrosshairPicking() {
        Method canHit = assertCanHitMethod();
        List<Integer> canHitOpcodes = canHitInstructionOpcodes();

        assertEquals(
            boolean.class,
            canHit.getReturnType(),
            "expected FaunaEntity.canHit to return boolean because crosshair picking reads this contract, actual: "
                + canHit.getReturnType()
        );
        assertEquals(
            FaunaEntity.class,
            canHit.getDeclaringClass(),
            "expected FaunaEntity to override canHit directly because the base Entity default is not hittable enough for fauna picking"
        );
        assertEquals(
            List.of(Opcodes.ICONST_1, Opcodes.IRETURN),
            canHitOpcodes,
            "expected FaunaEntity.canHit bytecode to return true because plain unit tests cannot bootstrap Minecraft Entity instances, actual opcodes: "
                + canHitOpcodes
        );
    }

    private static List<Integer> canHitInstructionOpcodes() {
        try (InputStream input = FaunaEntity.class.getResourceAsStream("FaunaEntity.class")) {
            if (input == null) {
                throw new AssertionError("expected FaunaEntity.class resource to be available for canHit contract test");
            }
            List<Integer> opcodes = new ArrayList<>();
            new ClassReader(input).accept(new ClassVisitor(Opcodes.ASM9) {
                @Override
                public MethodVisitor visitMethod(
                    int access,
                    String name,
                    String descriptor,
                    String signature,
                    String[] exceptions
                ) {
                    if (!"canHit".equals(name) || !"()Z".equals(descriptor)) {
                        return null;
                    }
                    return new MethodVisitor(Opcodes.ASM9) {
                        @Override
                        public void visitInsn(int opcode) {
                            opcodes.add(opcode);
                        }
                    };
                }
            }, ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
            return opcodes;
        } catch (IOException error) {
            throw new AssertionError(
                "expected to read FaunaEntity.class so canHit behavior can be tested without registry bootstrap",
                error
            );
        }
    }

    private static Method assertCanHitMethod() {
        try {
            return FaunaEntity.class.getDeclaredMethod("canHit");
        } catch (NoSuchMethodException error) {
            throw new AssertionError(
                "expected FaunaEntity.canHit override so fauna can participate in crosshair picking",
                error
            );
        }
    }

    private static void assertFaunaRawId(FaunaVisualKind kind, int expectedRawId) {
        assertEquals(
            expectedRawId,
            kind.expectedRawId(),
            "expected " + kind + " raw id to be " + expectedRawId
                + " because fauna entity raw-id order is fixed after whale and before modeled entities, actual: "
                + kind.expectedRawId()
        );
    }
}
