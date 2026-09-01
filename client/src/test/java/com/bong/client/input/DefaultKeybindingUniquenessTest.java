package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 全 client 默认键位唯一性契约。
 *
 * <p>测试扫描生产中的 R7 显式 {@code BongKeybindRegistry.BindingSpec} 声明；保留 direct
 * constructor 解析是为了让任何绕过 registry 的回归立即进入同一份物理冲突审计。
 * 未识别的默认键表达式会立即失败，防止新增绑定悄悄绕过全局冲突审计。UNKNOWN
 * 代表可由玩家重绑的入口，不占用物理键。默认值冲突已由 registry 收口；
 * {@code BotanyHudBootstrap.shouldCaptureSpellVolumeKey()} 仍作为运行时输入仲裁，
 * 防止玩家自定义成同一物理键时双重消费。
 */
class DefaultKeybindingUniquenessTest {
    private static final Path CLIENT_SOURCES = Path.of("src/main/java/com/bong/client");
    private static final Path COMBAT_KEYBINDINGS =
        CLIENT_SOURCES.resolve("combat/CombatKeybindings.java");
    private static final Path BOTANY_HUD =
        CLIENT_SOURCES.resolve("botany/BotanyHudBootstrap.java");
    private static final Path IDENTITY_BOOTSTRAP =
        CLIENT_SOURCES.resolve("identity/IdentityPanelScreenBootstrap.java");
    private static final Path VOID_ACTION_BOOTSTRAP =
        CLIENT_SOURCES.resolve("cultivation/voidaction/VoidActionScreenBootstrap.java");
    private static final Path FORGE_BOOTSTRAP =
        CLIENT_SOURCES.resolve("forge/ForgeScreenBootstrap.java");
    private static final Path EXTRACT_BOOTSTRAP =
        CLIENT_SOURCES.resolve("tsy/ExtractInteractionBootstrap.java");
    private static final Path KEYBIND_REGISTRY =
        CLIENT_SOURCES.resolve("input/BongKeybindRegistry.java");

    private static final Pattern DIRECT_KEY_BINDING =
        Pattern.compile("\\bnew\\s+KeyBinding\\s*\\(");
    private static final Pattern REGISTRY_SPEC =
        Pattern.compile("\\bnew\\s+(?:BongKeybindRegistry\\.)?BindingSpec\\s*\\(");
    private static final Pattern INT_CONSTANT = Pattern.compile(
        "\\b(?:(?:public|protected|private)\\s+)?static\\s+final\\s+int\\s+"
            + "([A-Za-z_$][A-Za-z0-9_$]*)\\s*=\\s*([^;]+);"
    );
    private static final Pattern STRING_CONSTANT = Pattern.compile(
        "\\b(?:(?:public|protected|private)\\s+)?static\\s+final\\s+String\\s+"
            + "([A-Za-z_$][A-Za-z0-9_$]*)\\s*=\\s*\"([^\"]*)\"\\s*;"
    );
    private static final Pattern GLFW_KEY =
        Pattern.compile("GLFW\\.GLFW_KEY_([A-Z0-9_]+)");
    private static final Pattern BINDING_OWNER = Pattern.compile(
        "new\\s+(?:BongKeybindRegistry\\.)?BindingOwner\\s*\\(\\s*(.+)\\s*\\)"
    );
    private static final Pattern RESERVED_DEFAULT = Pattern.compile(
        "new\\s+ReservedDefault\\s*\\(\\s*new\\s+BindingOwner\\s*\\(\\s*\"([^\"]+)\"\\s*\\)"
            + "\\s*,\\s*new\\s+PhysicalDefault\\s*\\(\\s*InputUtil\\.Type\\.([A-Z]+)"
            + "\\s*,\\s*GLFW\\.GLFW_KEY_([A-Z0-9_]+)\\s*\\)\\s*\\)"
    );

    private static final Map<PhysicalKey, List<String>> ALLOWED_DUPLICATES = Map.of();

    @Test
    void everyDefaultPhysicalKeyIsUniqueAfterRegistryMigration() throws IOException {
        Map<PhysicalKey, List<String>> duplicates = duplicateOwners(scanBindings());

        assertEquals(
            ALLOWED_DUPLICATES,
            duplicates,
            "默认物理键出现未列名冲突；所有 production default 必须由 registry 收口："
                + duplicates
        );
    }

    @Test
    void globalScanCoversDirectAndRegistryDeclarations() throws IOException {
        List<Binding> bindings = scanBindings();

        assertEquals(36, bindings.size(),
            "全局扫描必须覆盖当前 34 个 registry runtime binding 与 2 个 vanilla reservation");
        assertTrue(bindings.stream().anyMatch(binding ->
                binding.owner().equals("identity/IdentityPanelScreenBootstrap.java:identity.open_panel")),
            "全局扫描不能漏掉 BongKeybindRegistry.BindingSpec 声明");
        assertTrue(bindings.stream().anyMatch(binding ->
                binding.owner().equals("combat/CombatKeybindings.java:combat.quick_slot_9")),
            "全局扫描不能漏掉 registry 中带 F1+i 动态展开的快捷槽声明");
        assertTrue(bindings.stream().anyMatch(binding ->
                binding.owner().equals("input/BongKeybindRegistry.java:vanilla.chat")
                    && binding.key().equals(new PhysicalKey("KEYSYM", "T"))),
            "全局扫描必须将 registry 中的 vanilla chat reservation 纳入冲突审计");
        assertTrue(bindings.stream().anyMatch(binding ->
                binding.owner().equals("input/BongKeybindRegistry.java:vanilla.advancements")
                    && binding.key().equals(new PhysicalKey("KEYSYM", "L"))),
            "全局扫描必须将 registry 中的 vanilla advancements reservation 纳入冲突审计");
    }

    @Test
    void oHasOneExclusiveScreenOwnerAndUBelongsOnlyToExtractionCancel() throws IOException {
        List<Binding> bindings = scanBindings();

        assertEquals(
            List.of("identity/IdentityPanelScreenBootstrap.java:identity.open_panel"),
            ownersFor(bindings, new PhysicalKey("KEYSYM", "O")),
            "O 只能由身份面板一个独占 screen 入口拥有，不能再次触发化虚行动面板"
        );
        assertEquals(
            List.of("tsy/ExtractInteractionBootstrap.java:tsy.extract_cancel"),
            ownersFor(bindings, new PhysicalKey("KEYSYM", "U")),
            "U 在 extracting 时只能属于撤离取消，不能打开 Forge"
        );
        assertHasDefault(bindings,
            "cultivation/voidaction/VoidActionScreenBootstrap.java:void_action.open_screen", "UNKNOWN");
        assertHasDefault(bindings,
            "forge/ForgeScreenBootstrap.java:forge.open_screen", "UNKNOWN");

        String identity = codeOnly(read(IDENTITY_BOOTSTRAP));
        assertTrue(identity.contains("client.setScreen(create());"),
            "O 的唯一默认 owner 必须通过组合根打开身份面板这个独占 screen");
        String voidAction = codeOnly(read(VOID_ACTION_BOOTSTRAP));
        assertTrue(voidAction.contains("client.setScreen(new VoidActionScreen());"),
            "化虚行动保留可重绑入口时必须仍打开自己的独占 screen");
        String extract = compact(codeOnly(read(EXTRACT_BOOTSTRAP)));
        assertTrue(extract.contains("while(cancelKey.wasPressed()&&ExtractStateStore.snapshot().extracting())"),
            "撤离取消必须只在 extracting 状态消费 U 的 wasPressed 队列");
        assertTrue(extract.contains("ClientRequestSender.sendCancelExtract();"),
            "U 的唯一默认 owner 必须仍发送取消撤离请求");
        assertTrue(compact(codeOnly(read(FORGE_BOOTSTRAP)))
                .contains("InputUtil.UNKNOWN_KEY.getCode()"),
            "Forge 必须默认 UNKNOWN，不能在撤离期间抢占 U");
    }

    @Test
    void customPhysicalDuplicateKeepsItsDedicatedArbitrationGuard() {
        String combat = compact(codeOnly(read(COMBAT_KEYBINDINGS)));
        String botany = compact(codeOnly(read(BOTANY_HUD)));

        assertTrue(combat.contains("if(BotanyHudBootstrap.shouldCaptureSpellVolumeKey())"),
            "玩家自定义成同键时必须由 Combat→Botany 显式仲裁，不能恢复成两条独立消费链");
        assertTrue(botany.contains("returnHarvestSessionStore.capturesReservedInput()"),
            "R 双绑的仲裁结果必须由当前采集 session 状态驱动");
    }

    private static List<Binding> scanBindings() throws IOException {
        List<Binding> result = new ArrayList<>();
        try (var files = Files.walk(CLIENT_SOURCES)) {
            files.filter(Files::isRegularFile)
                .filter(path -> path.toString().endsWith(".java"))
                .sorted()
                .forEach(path -> collectBindings(path, result));
        }
        return result;
    }

    private static void collectBindings(Path path, List<Binding> result) {
        String source = codeOnly(read(path));
        Map<String, String> intConstants = localIntConstants(source);
        Map<String, String> stringConstants = localStringConstants(source);
        collectDirectBindings(path, source, intConstants, result);
        collectRegistryBindings(path, source, intConstants, stringConstants, result);
        if (path.equals(KEYBIND_REGISTRY)) {
            collectReservedDefaults(path, source, result);
        }
    }

    private static void collectReservedDefaults(Path path, String source, List<Binding> result) {
        Matcher matcher = RESERVED_DEFAULT.matcher(source);
        while (matcher.find()) {
            result.add(new Binding(
                new PhysicalKey(matcher.group(2), matcher.group(3)),
                relative(path) + ":" + matcher.group(1)
            ));
        }
    }

    private static void collectDirectBindings(
        Path path,
        String source,
        Map<String, String> intConstants,
        List<Binding> result
    ) {
        Matcher matcher = DIRECT_KEY_BINDING.matcher(source);
        int searchFrom = 0;
        while (matcher.find(searchFrom)) {
            int openParenthesis = matcher.end() - 1;
            int closeParenthesis = matchingParenthesis(source, openParenthesis);
            List<String> arguments = splitTopLevelArguments(
                source.substring(openParenthesis + 1, closeParenthesis)
            );
            if (arguments.size() != 4) {
                throw new IllegalStateException(
                    "无法解析 KeyBinding 四参数构造器: " + relative(path) + " -> " + arguments
                );
            }
            String owner = relative(path) + ":" + compact(arguments.get(0));
            String inputType = resolveInputType(arguments.get(1), path);
            for (String key : resolveKeys(arguments.get(2), intConstants, path, new HashSet<>())) {
                result.add(new Binding(new PhysicalKey(inputType, key), owner));
            }
            searchFrom = closeParenthesis + 1;
        }
    }

    private static void collectRegistryBindings(
        Path path,
        String source,
        Map<String, String> intConstants,
        Map<String, String> stringConstants,
        List<Binding> result
    ) {
        Matcher matcher = REGISTRY_SPEC.matcher(source);
        int searchFrom = 0;
        while (matcher.find(searchFrom)) {
            int openParenthesis = matcher.end() - 1;
            int closeParenthesis = matchingParenthesis(source, openParenthesis);
            List<String> arguments = splitTopLevelArguments(
                source.substring(openParenthesis + 1, closeParenthesis)
            );
            if (arguments.size() != 5) {
                throw new IllegalStateException(
                    "无法解析 BindingSpec 五参数构造器: " + relative(path) + " -> " + arguments
                );
            }
            Matcher ownerMatcher = BINDING_OWNER.matcher(arguments.get(0));
            if (!ownerMatcher.matches()) {
                throw new IllegalStateException(
                    "BindingSpec owner 必须是显式 BindingOwner: " + relative(path)
                        + " -> " + arguments.get(0)
                );
            }
            String inputType = resolveInputType(arguments.get(2), path);
            List<String> owners = resolveOwners(ownerMatcher.group(1), stringConstants, path);
            List<String> keys = resolveKeys(arguments.get(3), intConstants, path, new HashSet<>());
            if (owners.size() != keys.size()) {
                throw new IllegalStateException(
                    "BindingSpec owner/default 展开数量不一致: " + relative(path)
                        + " -> owners=" + owners + ", keys=" + keys
                );
            }
            for (int index = 0; index < owners.size(); index++) {
                result.add(new Binding(
                    new PhysicalKey(inputType, keys.get(index)),
                    relative(path) + ":" + owners.get(index)
                ));
            }
            searchFrom = closeParenthesis + 1;
        }
    }

    private static Map<String, String> localIntConstants(String source) {
        Map<String, String> constants = new HashMap<>();
        Matcher matcher = INT_CONSTANT.matcher(source);
        while (matcher.find()) {
            constants.put(matcher.group(1), matcher.group(2));
        }
        return constants;
    }

    private static Map<String, String> localStringConstants(String source) {
        Map<String, String> constants = new HashMap<>();
        Matcher matcher = STRING_CONSTANT.matcher(source);
        while (matcher.find()) {
            constants.put(matcher.group(1), matcher.group(2));
        }
        return constants;
    }

    private static String resolveString(
        String expression,
        Map<String, String> constants,
        Path path
    ) {
        String normalized = compact(expression);
        if (normalized.length() >= 2
            && normalized.startsWith("\"")
            && normalized.endsWith("\"")) {
            return normalized.substring(1, normalized.length() - 1);
        }
        if (constants.containsKey(normalized)) {
            return constants.get(normalized);
        }
        throw new IllegalStateException(
            "未识别的 BindingOwner 字符串表达式: " + relative(path) + " -> " + expression.trim()
        );
    }

    private static List<String> resolveOwners(
        String expression,
        Map<String, String> constants,
        Path path
    ) {
        String normalized = compact(expression);
        if (normalized.equals("\"combat.quick_slot_\"+(i+1)")) {
            return java.util.stream.IntStream.rangeClosed(1, 9)
                .mapToObj(index -> "combat.quick_slot_" + index)
                .toList();
        }
        return List.of(resolveString(expression, constants, path));
    }

    private static String resolveInputType(String expression, Path path) {
        return switch (compact(expression)) {
            case "InputUtil.Type.KEYSYM" -> "KEYSYM";
            case "InputUtil.Type.SCANCODE" -> "SCANCODE";
            case "InputUtil.Type.MOUSE" -> "MOUSE";
            default -> throw new IllegalStateException(
                "未识别的 KeyBinding 输入类型: " + relative(path) + " -> " + expression.trim()
            );
        };
    }

    private static List<String> resolveKeys(
        String expression,
        Map<String, String> constants,
        Path path,
        Set<String> resolving
    ) {
        String normalized = compact(expression);
        if (normalized.equals("GLFW.GLFW_KEY_UNKNOWN")
            || normalized.equals("InputUtil.UNKNOWN_KEY.getCode()")) {
            return List.of("UNKNOWN");
        }
        if (normalized.equals("GLFW.GLFW_KEY_F1+i")) {
            return List.of("F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9");
        }
        if (constants.containsKey(normalized)) {
            if (!resolving.add(normalized)) {
                throw new IllegalStateException(
                    "默认键常量循环引用: " + relative(path) + " -> " + resolving
                );
            }
            List<String> resolved = resolveKeys(constants.get(normalized), constants, path, resolving);
            resolving.remove(normalized);
            return resolved;
        }

        Matcher key = GLFW_KEY.matcher(normalized);
        if (key.matches()) {
            return List.of(key.group(1));
        }
        throw new IllegalStateException(
            "未识别的 KeyBinding 默认键表达式: " + relative(path) + " -> " + expression.trim()
        );
    }

    private static Map<PhysicalKey, List<String>> duplicateOwners(List<Binding> bindings) {
        Map<PhysicalKey, List<String>> ownersByKey = new TreeMap<>(
            Comparator.comparing(PhysicalKey::inputType).thenComparing(PhysicalKey::code)
        );
        for (Binding binding : bindings) {
            if (!binding.key().code().equals("UNKNOWN")) {
                ownersByKey.computeIfAbsent(binding.key(), ignored -> new ArrayList<>())
                    .add(binding.owner());
            }
        }

        Map<PhysicalKey, List<String>> duplicates = new TreeMap<>(
            Comparator.comparing(PhysicalKey::inputType).thenComparing(PhysicalKey::code)
        );
        ownersByKey.forEach((key, owners) -> {
            owners.sort(Comparator.naturalOrder());
            if (owners.size() > 1) {
                duplicates.put(key, List.copyOf(owners));
            }
        });
        return duplicates;
    }

    private static List<String> ownersFor(List<Binding> bindings, PhysicalKey key) {
        return bindings.stream()
            .filter(binding -> binding.key().equals(key))
            .map(Binding::owner)
            .sorted()
            .toList();
    }

    private static void assertHasDefault(List<Binding> bindings, String owner, String key) {
        assertTrue(bindings.stream().anyMatch(binding ->
                binding.owner().equals(owner) && binding.key().code().equals(key)),
            owner + " 必须明确注册默认键 " + key);
    }

    private static int matchingParenthesis(String source, int openParenthesis) {
        int depth = 0;
        boolean inString = false;
        boolean inCharacter = false;
        boolean escaped = false;
        for (int index = openParenthesis; index < source.length(); index++) {
            char current = source.charAt(index);
            if (escaped) {
                escaped = false;
                continue;
            }
            if ((inString || inCharacter) && current == '\\') {
                escaped = true;
                continue;
            }
            if (!inCharacter && current == '"') {
                inString = !inString;
                continue;
            }
            if (!inString && current == '\'') {
                inCharacter = !inCharacter;
                continue;
            }
            if (inString || inCharacter) {
                continue;
            }
            if (current == '(') {
                depth++;
            } else if (current == ')' && --depth == 0) {
                return index;
            }
        }
        throw new IllegalStateException("KeyBinding 构造器括号未闭合");
    }

    private static List<String> splitTopLevelArguments(String arguments) {
        List<String> result = new ArrayList<>();
        int start = 0;
        int depth = 0;
        boolean inString = false;
        boolean inCharacter = false;
        boolean escaped = false;
        for (int index = 0; index < arguments.length(); index++) {
            char current = arguments.charAt(index);
            if (escaped) {
                escaped = false;
                continue;
            }
            if ((inString || inCharacter) && current == '\\') {
                escaped = true;
                continue;
            }
            if (!inCharacter && current == '"') {
                inString = !inString;
                continue;
            }
            if (!inString && current == '\'') {
                inCharacter = !inCharacter;
                continue;
            }
            if (inString || inCharacter) {
                continue;
            }
            if (current == '(' || current == '[' || current == '{') {
                depth++;
            } else if (current == ')' || current == ']' || current == '}') {
                depth--;
            } else if (current == ',' && depth == 0) {
                result.add(arguments.substring(start, index).trim());
                start = index + 1;
            }
        }
        result.add(arguments.substring(start).trim());
        return result;
    }

    private static String relative(Path path) {
        return CLIENT_SOURCES.relativize(path).toString().replace('\\', '/');
    }

    private static String codeOnly(String source) {
        return source
            .replaceAll("(?s)/\\*.*?\\*/", "")
            .replaceAll("(?m)//.*$", "");
    }

    private static String compact(String source) {
        return source.replaceAll("\\s+", "");
    }

    private static String read(Path path) {
        try {
            return Files.readString(path);
        } catch (IOException exception) {
            throw new IllegalStateException(exception);
        }
    }

    private record PhysicalKey(String inputType, String code)
        implements Comparable<PhysicalKey> {
        @Override
        public int compareTo(PhysicalKey other) {
            int type = inputType.compareTo(other.inputType);
            return type != 0 ? type : code.compareTo(other.code);
        }
    }

    private record Binding(PhysicalKey key, String owner) {
    }
}
