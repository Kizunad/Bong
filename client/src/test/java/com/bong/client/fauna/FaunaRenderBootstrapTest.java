package com.bong.client.fauna;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

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
        assertFaunaRawId(FaunaVisualKind.GREEN_SPIDER, 135);
        assertFaunaRawId(FaunaVisualKind.JUNGLE_SCORPION, 136);
        assertFaunaRawId(FaunaVisualKind.COCKADE_SNAKE, 137);
        assertFaunaRawId(FaunaVisualKind.BLUE_SPIDER, 138);
        assertFaunaRawId(FaunaVisualKind.ICE_SCORPION, 139);
        assertFaunaRawId(FaunaVisualKind.MANDRAKE_SNAKE, 140);
        assertFaunaRawId(FaunaVisualKind.DARK_TIGER, 141);
        assertFaunaRawId(FaunaVisualKind.LIVING_PILLAR, 142);
        assertFaunaRawId(FaunaVisualKind.POISON_DRAGON, 143);
        assertFaunaRawId(FaunaVisualKind.BONE_DRAGON, 144);
        assertFaunaRawId(FaunaVisualKind.HEIWUSHI, 145);
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
            "geo/green_spider.geo.json",
            "geo/jungle_scorpion.geo.json",
            "geo/cockade_snake.geo.json",
            "geo/blue_spider.geo.json",
            "geo/ice_scorpion.geo.json",
            "geo/mandrake_snake.geo.json",
            "geo/dark_tiger.geo.json",
            "geo/living_pillar.geo.json",
            "geo/poison_dragon.geo.json",
            "geo/bone_dragon.geo.json",
            "geo/heiwushi.geo.json",
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

    // ─── 野兽 T-Pose 修复：controller 按物种派生 idle 动画名（此前硬编码 animation.fauna.idle，
    //     专属模型物种加载各自动画文件→无此 key→GeckoLib 解析不到→定格绑定姿势/T-Pose）───

    @Test
    void idleAnimationNameDerivesFromAnimPath() {
        // 通用 fauna 模型（animPath==null）→ animation.fauna.idle（在 fauna.animation.json 内）
        assertEquals("animation.fauna.idle", FaunaVisualKind.DEVOUR_RAT.idleAnimationName(),
            "通用 fauna 物种应回退 animation.fauna.idle");
        assertEquals("animation.fauna.idle", FaunaVisualKind.HEIWUSHI.idleAnimationName(),
            "通用 fauna 物种应回退 animation.fauna.idle");
        // 专属模型（animPath!=null）→ animation.bong.<animPath>.idle（在各物种文件内）
        assertEquals("animation.bong.green_spider.idle", FaunaVisualKind.GREEN_SPIDER.idleAnimationName());
        assertEquals("animation.bong.ice_scorpion.idle", FaunaVisualKind.ICE_SCORPION.idleAnimationName());
        assertEquals("animation.bong.bone_dragon.idle", FaunaVisualKind.BONE_DRAGON.idleAnimationName());
    }

    @Test
    void everyFaunaSpeciesAnimationFileContainsItsIdleAnimation() throws IOException {
        // 不变式（防 T-Pose 复发）：每个 FaunaVisualKind 的 animationId() 指向的动画文件必须存在，
        // 且含 idleAnimationName() 对应的 key——否则 controller setAnimation 解析不到 → 实体定格 T-Pose。
        // 这条用例同时锁住 ice_scorpion 缺 idle（修前只有 attack）和未来新物种漏配 idle。
        Path resources = Path.of("src", "main", "resources");
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            String animFile = kind.animationId().getPath(); // animations/<x>.animation.json
            Path path = resources.resolve("assets/bong").resolve(animFile);
            assertTrue(Files.isRegularFile(path), kind + " 动画文件缺失：" + path);

            JsonObject root = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
            JsonObject animations = root.getAsJsonObject("animations");
            String idleName = kind.idleAnimationName();
            assertTrue(
                animations != null && animations.has(idleName),
                kind + " 的动画文件 " + animFile + " 缺少 idle 动画 \"" + idleName
                    + "\"——controller 会解析不到 → 实体定格 T-Pose。文件含: "
                    + (animations == null ? "<no animations obj>" : animations.keySet())
            );
        }
    }

    @Test
    void faunaEntityDerivesIdleFromVisualKindAndDropsHardcodedFaunaIdle() {
        // 字节码核验 controller 接线（无法 bootstrap GeckoLib 运行时）：
        // ① FaunaEntity 必须调用 FaunaVisualKind.idleAnimationName()（按物种取 idle）；
        // ② FaunaEntity 不得再硬编码字面量 "animation.fauna.idle"（已下沉到 FaunaVisualKind）。
        // 二者同时成立才能保证专属模型物种不再 T-Pose；任一回退都撞红。
        Set<String> ldcStrings = new HashSet<>();
        Set<String> invokedMethods = new HashSet<>();
        try (InputStream input = FaunaEntity.class.getResourceAsStream("FaunaEntity.class")) {
            if (input == null) {
                throw new AssertionError("expected FaunaEntity.class resource for idle-wiring bytecode test");
            }
            new ClassReader(input).accept(new ClassVisitor(Opcodes.ASM9) {
                @Override
                public MethodVisitor visitMethod(
                    int access,
                    String name,
                    String descriptor,
                    String signature,
                    String[] exceptions
                ) {
                    return new MethodVisitor(Opcodes.ASM9) {
                        @Override
                        public void visitLdcInsn(Object value) {
                            if (value instanceof String s) {
                                ldcStrings.add(s);
                            }
                        }

                        @Override
                        public void visitMethodInsn(
                            int opcode,
                            String owner,
                            String methodName,
                            String methodDescriptor,
                            boolean isInterface
                        ) {
                            invokedMethods.add(methodName);
                        }
                    };
                }
            }, ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
        } catch (IOException error) {
            throw new AssertionError("expected to read FaunaEntity.class for idle-wiring test", error);
        }

        assertTrue(
            invokedMethods.contains("idleAnimationName"),
            "expected FaunaEntity.registerControllers to call FaunaVisualKind.idleAnimationName() so each "
                + "species loops its OWN idle（否则专属模型物种定格 T-Pose），actual invoked methods: " + invokedMethods
        );
        assertFalse(
            ldcStrings.contains("animation.fauna.idle"),
            "expected FaunaEntity to NOT hardcode \"animation.fauna.idle\"（已下沉到 FaunaVisualKind."
                + "idleAnimationName）；若该字面量重现说明 controller 又写死了通用 idle → 专属物种 T-Pose，"
                + "actual LDC strings: " + ldcStrings
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
