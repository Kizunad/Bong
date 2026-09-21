package com.bong.client.weapon;

import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class WeaponAssetReferenceIntegrityTest {
    private static final Path RESOURCES = Path.of("src", "main", "resources");
    private static final Path ITEM_MODELS = RESOURCES.resolve("assets")
        .resolve("bong")
        .resolve("models")
        .resolve("item");

    @Test
    void everyPublishedObjBindsPublishedMaterialsAndTextures() throws Exception {
        List<Path> objectFiles;
        try (Stream<Path> itemDirectories = Files.list(ITEM_MODELS)) {
            objectFiles = itemDirectories
                .filter(Files::isDirectory)
                .map(directory -> directory.resolve(directory.getFileName() + ".obj"))
                .filter(Files::isRegularFile)
                .sorted()
                .toList();
        }

        assertFalse(objectFiles.isEmpty(), "已发布武器 OBJ 目录为空，资产引用测试没有实际覆盖对象");

        for (Path obj : objectFiles) {
            String itemId = obj.getFileName().toString().replaceFirst("\\.obj$", "");
            Path itemDirectory = obj.getParent();
            Path expectedMtl = itemDirectory.resolve(itemId + ".mtl");
            List<String> objLines = Files.readAllLines(obj, StandardCharsets.UTF_8);

            assertTrue(objLines.stream().anyMatch(line -> line.trim().equals("mtllib " + itemId + ".mtl")),
                "item=" + itemId + " OBJ 未声明 mtllib " + itemId + ".mtl");
            assertTrue(Files.isRegularFile(expectedMtl),
                "item=" + itemId + " OBJ 引用的 MTL 文件不存在: " + expectedMtl);

            Set<String> usedMaterials = directiveValues(objLines, "usemtl");
            assertFalse(usedMaterials.isEmpty(),
                "item=" + itemId + " OBJ 没有任何 usemtl，加载器无法绑定材质");

            List<String> mtlLines = Files.readAllLines(expectedMtl, StandardCharsets.UTF_8);
            Set<String> declaredMaterials = directiveValues(mtlLines, "newmtl");
            for (String usedMaterial : usedMaterials) {
                assertTrue(declaredMaterials.contains(usedMaterial),
                    "item=" + itemId + " usemtl=" + usedMaterial
                        + " 未在 " + expectedMtl + " 的 newmtl 中声明");
            }

            List<String> textureReferences = mtlLines.stream()
                .map(String::trim)
                .filter(line -> line.startsWith("map_Kd "))
                .map(line -> line.substring("map_Kd ".length()).trim())
                .map(WeaponAssetReferenceIntegrityTest::lastToken)
                .toList();
            assertFalse(textureReferences.isEmpty(),
                "item=" + itemId + " MTL 没有 map_Kd，材质没有贴图引用");
            for (String textureReference : textureReferences) {
                int separator = textureReference.indexOf(':');
                assertTrue(separator > 0 && separator < textureReference.length() - 1,
                    "item=" + itemId + " map_Kd=" + textureReference + " 不是合法 namespaced 贴图引用");

                String namespace = textureReference.substring(0, separator);
                String texturePath = textureReference.substring(separator + 1);
                if (!texturePath.endsWith(".png")) {
                    texturePath += ".png";
                }
                Path texture = RESOURCES.resolve("assets")
                    .resolve(namespace)
                    .resolve("textures")
                    .resolve(texturePath);
                assertTrue(Files.isRegularFile(texture),
                    "item=" + itemId + " map_Kd=" + textureReference
                        + " 指向的贴图文件不存在: " + texture);
            }
        }
    }

    private static Set<String> directiveValues(List<String> lines, String directive) {
        Set<String> values = new HashSet<>();
        String prefix = directive + " ";
        for (String line : lines) {
            String trimmed = line.trim();
            if (trimmed.startsWith(prefix)) {
                String value = trimmed.substring(prefix.length()).trim();
                if (!value.isEmpty()) {
                    values.add(value);
                }
            }
        }
        return values;
    }

    private static String lastToken(String value) {
        String[] tokens = value.trim().split("\\s+");
        return tokens[tokens.length - 1];
    }
}
