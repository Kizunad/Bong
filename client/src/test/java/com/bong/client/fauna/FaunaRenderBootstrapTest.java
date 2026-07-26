package com.bong.client.fauna;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
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
        assertEquals("animation.fauna.idle", FaunaVisualKind.ASH_SPIDER.idleAnimationName(),
            "通用 fauna 物种应回退 animation.fauna.idle");
        // 噬元鼠改走专属模型（devour_rat.geo.json + devour_rat.animation.json，含 idle/walk/run/peck/claw/pounce），
        // idle 应取 animation.bong.devour_rat.idle 而非通用回退。
        assertEquals("animation.bong.devour_rat.idle", FaunaVisualKind.DEVOUR_RAT.idleAnimationName(),
            "噬元鼠走专属模型动画文件，idle 应取 animation.bong.devour_rat.idle");
        // 专属模型（animPath!=null）→ animation.bong.<animPath>.idle（在各物种文件内）
        // 黑武士现走专属 heiwushi.animation.json（boss 招式动画 dark_barrage/dark_vortex/transform
        // 都在该文件，idle 同理），故 idle 名应为 animation.bong.heiwushi.idle 而非通用回退。
        assertEquals("animation.bong.heiwushi.idle", FaunaVisualKind.HEIWUSHI.idleAnimationName(),
            "黑武士走专属模型动画文件，idle 应取 animation.bong.heiwushi.idle");
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

    // ─── 移动动画：controller 按水平速度切 idle↔walk↔run（此前只播 idle，
    //     walk/run 是死资产从不触发）。噬元鼠接入三态，其余物种维持 null（idle-only）───

    @Test
    void walkRunAnimationNamesOnlyForDevourRat() {
        // 噬元鼠接入 walk/run，名字与专属动画文件的 key 对齐。
        assertEquals("animation.bong.devour_rat.walk", FaunaVisualKind.DEVOUR_RAT.walkAnimationName(),
            "噬元鼠 walk 名应对齐 animation.bong.devour_rat.walk");
        assertEquals("animation.bong.devour_rat.run", FaunaVisualKind.DEVOUR_RAT.runAnimationName(),
            "噬元鼠 run 名应对齐 animation.bong.devour_rat.run");
        // 其它物种暂未接入移动动画：controller 读到 null 会退回 idle，绝不能返回一个
        // 动画文件里不存在的 key（否则 GeckoLib 解析失败 → T-Pose）。
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            if (kind == FaunaVisualKind.DEVOUR_RAT) {
                continue;
            }
            assertEquals(null, kind.walkAnimationName(), kind + " 尚未接入 walk，应返回 null");
            assertEquals(null, kind.runAnimationName(), kind + " 尚未接入 run，应返回 null");
        }
    }

    @Test
    void facesMovementDirectionOnlyForDevourRat() {
        assertTrue(FaunaVisualKind.DEVOUR_RAT.facesMovementDirection(),
            "噬元鼠是 marker（不下发 yaw），须用客户端自算移动朝向");
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            if (kind == FaunaVisualKind.DEVOUR_RAT) {
                continue;
            }
            assertTrue(!kind.facesMovementDirection(), kind + " 未接入客户端移动朝向");
        }
    }

    @Test
    void approachYawTurnsTowardTargetWithClampAndWrap() {
        // 小于步长：一步到位。
        assertEquals(30.0f, FaunaYawMath.approachYaw(0.0f, 30.0f, 90.0f), 1e-4);
        // 大于步长且走近路：target=200° 归一化 = −160°，从 0 最短转向为负 → 只走 −maxDelta。
        assertEquals(-28.0f, FaunaYawMath.approachYaw(0.0f, 200.0f, 28.0f), 1e-4);
        // 正向大角：target=120°（>maxDelta），正向走 +28。
        assertEquals(28.0f, FaunaYawMath.approachYaw(0.0f, 120.0f, 28.0f), 1e-4);
        // 环绕：从 170 转向 -170（差 20°，走近路 +20 越过 180）。
        assertEquals(-170.0f, FaunaYawMath.approachYaw(170.0f, -170.0f, 90.0f), 1e-4);
        // wrapDegrees 边界。
        assertEquals(-179.0f, FaunaYawMath.wrapDegrees(181.0f), 1e-4);
        assertEquals(180.0f, FaunaYawMath.wrapDegrees(180.0f), 1e-4);
        assertEquals(0.0f, FaunaYawMath.wrapDegrees(360.0f), 1e-4);
    }

    @Test
    void devourRatAnimationFileContainsWalkAndRun() throws IOException {
        // 不变式：既然 controller 会对噬元鼠 setAnimation(walk/run)，其动画文件必须含这两个 key，
        // 否则移动时 GeckoLib 解析不到 → 定格 T-Pose（与 idle 缺失同源的坑）。
        Path resources = Path.of("src", "main", "resources");
        Path path = resources.resolve("assets/bong")
            .resolve(FaunaVisualKind.DEVOUR_RAT.animationId().getPath());
        JsonObject animations = JsonParser.parseString(Files.readString(path))
            .getAsJsonObject().getAsJsonObject("animations");
        for (String name : new String[] {"animation.bong.devour_rat.walk", "animation.bong.devour_rat.run"}) {
            assertTrue(animations != null && animations.has(name),
                "噬元鼠动画文件缺少移动动画 \"" + name + "\"——移动时会定格 T-Pose。文件含: "
                    + (animations == null ? "<none>" : animations.keySet()));
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

    /**
     * 全仓不变式：**任何** GeckoLib geo 文件里都不得出现负 {@code size} 的 cube。
     *
     * <p>Blockbench 允许某轴 {@code to < from}（手雕时很容易出现），转换脚本若直接
     * {@code to - from} 就会写出负 size。Bedrock geometry 约定 size 非负，负宽度会让该 cube
     * 退化 / 翻面——背面剔除后从外面直接看不见，而加载、解析、渲染全程**不报任何错**，
     * 只能靠实机盯出来。
     *
     * <p>实战命中：噬元鼠尾脊末段 {@code ridge3}（`to.x < from.x`，size.x = -0.186）正是
     * q2 满档才点亮的第 4 段蓝脊；不修则"吸饱了"这一档的最后一段可能整段不可见，而所有
     * 贴图/档位/发光测试照样全绿。
     */
    @Test
    void noFaunaGeoModelContainsNegativeCubeSize() throws IOException {
        Path geoDir = Path.of("src", "main", "resources", "assets", "bong", "geo");
        assertTrue(Files.isDirectory(geoDir), "geo 资源目录应存在：" + geoDir);

        List<String> offenders = new ArrayList<>();
        List<Path> geoFiles;
        try (var stream = Files.list(geoDir)) {
            geoFiles = stream.filter(p -> p.getFileName().toString().endsWith(".geo.json"))
                .sorted()
                .toList();
        }
        assertFalse(geoFiles.isEmpty(), "geo 目录不应为空，否则本不变式形同虚设");

        for (Path file : geoFiles) {
            JsonObject root = JsonParser.parseString(Files.readString(file)).getAsJsonObject();
            if (!root.has("minecraft:geometry")) {
                continue;
            }
            for (JsonElement geometry : root.getAsJsonArray("minecraft:geometry")) {
                JsonObject geo = geometry.getAsJsonObject();
                if (!geo.has("bones")) {
                    continue;
                }
                for (JsonElement boneElement : geo.getAsJsonArray("bones")) {
                    JsonObject bone = boneElement.getAsJsonObject();
                    if (!bone.has("cubes")) {
                        continue;
                    }
                    for (JsonElement cubeElement : bone.getAsJsonArray("cubes")) {
                        JsonObject cube = cubeElement.getAsJsonObject();
                        if (!cube.has("size")) {
                            continue;
                        }
                        JsonArray size = cube.getAsJsonArray("size");
                        for (int axis = 0; axis < size.size(); axis++) {
                            if (size.get(axis).getAsDouble() < 0) {
                                offenders.add(file.getFileName() + " bone=" + bone.get("name")
                                    + " size=" + size + "（轴 " + axis + " 为负）");
                            }
                        }
                    }
                }
            }
        }

        assertTrue(
            offenders.isEmpty(),
            "以下 cube 的 size 含负值 —— 会退化/翻面且全程不报错，只能实机盯出来；"
                + "转换脚本应取 min(from,to) 作 origin、abs(to-from) 作 size：\n  "
                + String.join("\n  ", offenders)
        );
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
