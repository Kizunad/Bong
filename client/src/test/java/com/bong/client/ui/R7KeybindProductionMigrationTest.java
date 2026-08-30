package com.bong.client.ui;

import com.bong.client.input.BongKeybindRegistry;
import com.sun.source.tree.AssignmentTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import javax.lang.model.element.Element;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.VariableElement;
import java.io.IOException;
import java.net.URISyntaxException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.UnaryOperator;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7KeybindProductionMigrationTest {
    private static final Path CLIENT_ROOT = R7SourceScan.productionRoot();
    private static final String REGISTRATION_CALL = "BongKeybindRegistry.global().register(";
    private static final String CATEGORY = "category.bong-client.controls";
    private static final Set<String> TARGET_FILES = Set.of(
        "identity/IdentityPanelScreenBootstrap.java",
        "cultivation/voidaction/VoidActionScreenBootstrap.java",
        "forge/ForgeScreenBootstrap.java",
        "tsy/ExtractInteractionBootstrap.java",
        "lingtian/LingtianActionScreenBootstrap.java",
        "spirittreasure/SpiritTreasureScreenBootstrap.java",
        "dying_elder/DyingElderInteractionKeybindings.java"
    );
    private static final Set<String> TARGET_OWNER_IDS = Set.of(
        "identity.open_panel",
        "void_action.open_screen",
        "forge.open_screen",
        "tsy.extract_start",
        "tsy.extract_cancel",
        "lingtian.open_action_screen",
        "spirittreasure.open_screen",
        "dying_elder.give_dan",
        "dying_elder.refuse",
        "dying_elder.delay"
    );

    @Test
    void everyTargetProductionSiteUsesTheGlobalRegistryOnly() throws IOException {
        Map<String, Integer> expectedCallCount = Map.of(
            "identity/IdentityPanelScreenBootstrap.java", 1,
            "cultivation/voidaction/VoidActionScreenBootstrap.java", 1,
            "forge/ForgeScreenBootstrap.java", 1,
            "tsy/ExtractInteractionBootstrap.java", 2,
            "lingtian/LingtianActionScreenBootstrap.java", 1,
            "spirittreasure/SpiritTreasureScreenBootstrap.java", 1,
            "dying_elder/DyingElderInteractionKeybindings.java", 3
        );

        for (String relativePath : TARGET_FILES) {
            String source = R7SourceScan.read(CLIENT_ROOT.resolve(relativePath));
            assertEquals(expectedCallCount.get(relativePath), occurrences(source, REGISTRATION_CALL),
                "每个 R7 P1 目标文件的绑定都必须经过 global registry：" + relativePath);
            assertFalse(source.contains("KeyBindingHelper"),
                "迁移后的生产文件不得残留 KeyBindingHelper：" + relativePath);
            assertFalse(source.contains("new KeyBinding"),
                "迁移后的生产文件不得绕过 registry 直接构造 KeyBinding：" + relativePath);
        }

        List<ProductionBinding> actual = productionBindings();
        assertEquals(10, actual.size(), "七个入口应迁移恰好十个生产绑定");
        assertEquals(TARGET_OWNER_IDS, actual.stream()
            .map(ProductionBinding::ownerId)
            .collect(Collectors.toSet()),
            "R7 P1 迁移的 owner id 集合必须完整且无遗漏");
    }

    @Test
    void registryContractsMatchTheFrozenProductionSiteFixture() throws IOException {
        Map<String, FixtureRow> expectedBySite = fixtureRows().stream()
            .filter(row -> TARGET_OWNER_IDS.contains(row.ownerId()))
            .collect(Collectors.toMap(FixtureRow::siteKey, row -> row));
        assertEquals(TARGET_OWNER_IDS, expectedBySite.values().stream()
            .map(FixtureRow::ownerId)
            .collect(Collectors.toSet()),
            "production-sites fixture 必须为每个迁移 owner 提供唯一冻结行");

        for (ProductionBinding actual : productionBindings()) {
            FixtureRow expected = expectedBySite.get(actual.siteKey());
            assertNotNull(expected, "fixture 缺少迁移站点：" + actual.siteKey());
            assertEquals(expected.ownerId(), actual.ownerId(),
                "owner id 必须与 keybind-production-sites.tsv 对拍：" + actual.siteKey());
            assertEquals(expected.translationKey(), actual.translationKey(),
                "translation key 必须与冻结 fixture 对拍：" + actual.ownerId());
            assertEquals(expected.inputType(), actual.inputType(),
                "InputUtil.Type 必须与冻结 fixture 对拍：" + actual.ownerId());
            assertEquals(expected.normalizedDefaultContract(), actual.defaultContract(),
                "默认键必须与冻结 fixture 对拍：" + actual.ownerId());
            assertEquals(expected.category(), actual.category(),
                "category 必须与冻结 fixture 对拍：" + actual.ownerId());
        }
    }

    @Test
    void forgeLegacyMigrationIsRegisteredBeforeItsEndTickConsumer() throws IOException {
        String source = R7SourceScan.read(CLIENT_ROOT.resolve(
            "forge/ForgeScreenBootstrap.java"
        ));
        int lifecycleRegistration = source.indexOf(
            "ClientLifecycleEvents.CLIENT_STARTED.register(ForgeScreenBootstrap::migrateLegacyBinding)"
        );
        int endTickRegistration = source.indexOf(
            "ClientTickEvents.END_CLIENT_TICK.register(ForgeScreenBootstrap::onEndClientTick)"
        );

        assertTrue(lifecycleRegistration >= 0,
            "Forge 必须在客户端首个 tick 前注册存量键位迁移回调");
        assertTrue(endTickRegistration > lifecycleRegistration,
            "Forge 的旧键位迁移接线必须先于 wasPressed 消费回调声明");
        assertTrue(source.contains("migrationService().migrateOnce("),
            "Forge 必须通过 input 应用服务编排一次性存量迁移");
        assertTrue(source.contains("BongKeybindRegistry.global().migrateLegacyBoundKey("),
            "迁移 action 必须继续通过按 translation key 的 registry seam 改绑");
        assertTrue(source.contains("LEGACY_MIGRATION_ID")
                && source.contains("forge-open-screen-u-v1"),
            "Forge 的存量迁移 marker 必须带稳定版本 id");
        assertTrue(source.contains("KeybindMigrationService.clientConfig()"),
            "Forge 必须依赖迁移应用服务，不能自行编排 marker 状态");
        assertFalse(source.contains("KeybindMigrationPersistence")
                || source.contains("FabricLoader")
                || source.contains("getConfigDir()"),
            "Forge UI bootstrap 不得依赖迁移持久化接口或文件存储细节");
        assertTrue(source.contains("client.options::setKeyCode"),
            "迁移必须通过 GameOptions 持久化 UNKNOWN，而不是只改内存字段");
        assertTrue(source.contains("client.options.setKeyCode(keyBinding(), LEGACY_DEFAULT_KEY)")
                && source.contains("KeyBinding.updateKeysByCode()"),
            "marker 写失败时必须恢复旧 U 并刷新物理索引，不能留下未提交的 UNKNOWN 状态");
        assertTrue(source.contains("while (keyBinding().wasPressed())")
                && source.contains("if (ClientInputPolicy.shouldDispatchForgeOpen())"),
            "Forge 必须先排空历史 U 的按键队列，再以 extracting 状态仲裁开屏，不能延迟重放");
        assertTrue(source.contains("ClientInputPolicy.shouldDispatchForgeOpen()"),
            "Forge 必须通过客户端共享输入仲裁策略执行 extracting 仲裁，不能直接依赖 TSY store");
        assertTrue(source.contains("catch (IllegalStateException exception)"),
            "CLIENT_STARTED 边界必须捕获 marker I/O 故障，不能阻断客户端启动");
        assertTrue(source.contains("BongClient.LOGGER.error(")
                && source.contains("migration was rolled back")
                && source.contains("client startup will continue"),
            "迁移持久化故障必须留下可观测 error，并明确回滚后继续启动");
    }

    @Test
    void voidActionLegacyMigrationIsRegisteredBeforeItsEndTickConsumer() throws IOException {
        String source = R7SourceScan.read(CLIENT_ROOT.resolve(
            "cultivation/voidaction/VoidActionScreenBootstrap.java"
        ));
        int lifecycleRegistration = source.indexOf(
            "ClientLifecycleEvents.CLIENT_STARTED.register(VoidActionScreenBootstrap::migrateLegacyBinding)"
        );
        int endTickRegistration = source.indexOf(
            "ClientTickEvents.END_CLIENT_TICK.register(VoidActionScreenBootstrap::onEndClientTick)"
        );

        assertTrue(lifecycleRegistration >= 0,
            "VoidAction 必须在客户端首个 tick 前注册 O 存量键位迁移回调");
        assertTrue(endTickRegistration > lifecycleRegistration,
            "VoidAction 的旧 O 键迁移接线必须先于 wasPressed 消费回调声明");
        assertTrue(source.contains("migrationService().migrateOnce("),
            "VoidAction 必须通过 input 应用服务编排一次性存量迁移");
        assertTrue(source.contains("BongKeybindRegistry.global().migrateLegacyBoundKey("),
            "VoidAction 迁移 action 必须继续通过按 translation key 的 registry seam 改绑");
        assertTrue(source.contains("LEGACY_MIGRATION_ID")
                && source.contains("void-action-open-screen-o-v1"),
            "VoidAction 的存量迁移 marker 必须带稳定版本 id");
        assertTrue(source.contains("KeybindMigrationService.clientConfig()"),
            "VoidAction 必须依赖迁移应用服务，不能自行编排 marker 状态");
        assertFalse(source.contains("KeybindMigrationPersistence")
                || source.contains("FabricLoader")
                || source.contains("getConfigDir()"),
            "VoidAction bootstrap 不得依赖迁移持久化接口或文件存储细节");
        assertTrue(source.contains("client.options::setKeyCode"),
            "VoidAction 迁移必须通过 GameOptions 持久化 UNKNOWN");
        assertTrue(source.contains("client.options.setKeyCode(keyBinding(), LEGACY_DEFAULT_KEY)")
                && source.contains("KeyBinding.updateKeysByCode()"),
            "VoidAction marker 写失败时必须恢复旧 O 并刷新物理索引");
        assertTrue(source.contains("GLFW.GLFW_KEY_O"),
            "VoidAction 迁移必须只处理旧默认 O，不覆盖玩家自定义键位");
        assertTrue(source.contains("catch (IllegalStateException exception)"),
            "VoidAction CLIENT_STARTED 边界必须捕获 marker I/O 故障");
    }

    @Test
    void theOUConflictClusterConvergesThroughRegistryRulesAndUnknownIsUnbound() {
        BongKeybindRegistry registry = testRegistry();
        registry.register(spec("forge.open_screen", "key.r7.forge", GLFW.GLFW_KEY_U));
        registry.register(spec("identity.open_panel", "key.r7.identity", GLFW.GLFW_KEY_O));
        registry.register(spec("void_action.open_screen", "key.r7.void_action", InputUtil.UNKNOWN_KEY.getCode()));
        registry.register(spec("tsy.extract_start", "key.r7.extract_start", GLFW.GLFW_KEY_Y));
        registry.register(spec("tsy.extract_cancel", "key.r7.extract_cancel", InputUtil.UNKNOWN_KEY.getCode()));

        assertEquals(List.of(
            "forge.open_screen",
            "identity.open_panel",
            "void_action.open_screen",
            "tsy.extract_start",
            "tsy.extract_cancel"
        ), registry.registrations().stream()
            .map(registration -> registration.owner().id())
            .toList(),
            "成功注册顺序必须保留 BongClient 中 forge→identity→void→extract start→cancel 的顺序");

        BongKeybindRegistry oConflict = testRegistry();
        oConflict.register(spec("identity.open_panel", "key.r7.identity.conflict", GLFW.GLFW_KEY_O));
        IllegalArgumentException oFailure = assertThrows(IllegalArgumentException.class,
            () -> oConflict.register(spec("void_action.open_screen", "key.r7.void.conflict", GLFW.GLFW_KEY_O)),
            "void action 若恢复 O 默认键，必须由 registry 拒绝 O 物理冲突");
        assertTrue(oFailure.getMessage().contains("identity.open_panel")
                && oFailure.getMessage().contains("void_action.open_screen"),
            "O 冲突错误必须同时指明两个 owner，便于定位迁移回归");

        BongKeybindRegistry uConflict = testRegistry();
        uConflict.register(spec("forge.open_screen", "key.r7.forge.conflict", GLFW.GLFW_KEY_U));
        IllegalArgumentException uFailure = assertThrows(IllegalArgumentException.class,
            () -> uConflict.register(spec("tsy.extract_cancel", "key.r7.cancel.conflict", GLFW.GLFW_KEY_U)),
            "extract cancel 若恢复 U 默认键，必须由 registry 拒绝 U 物理冲突");
        assertTrue(uFailure.getMessage().contains("forge.open_screen")
                && uFailure.getMessage().contains("tsy.extract_cancel"),
            "U 冲突错误必须同时指明两个 owner，便于定位迁移回归");

        BongKeybindRegistry unknowns = testRegistry();
        unknowns.register(spec("void_action.open_screen", "key.r7.void.unknown.one", InputUtil.UNKNOWN_KEY.getCode()));
        unknowns.register(spec("tsy.extract_cancel", "key.r7.cancel.unknown.two", InputUtil.UNKNOWN_KEY.getCode()));
        assertEquals(2, unknowns.registrations().size(),
            "UNKNOWN 没有物理键身份，两个冻结 UNKNOWN 绑定都必须成功注册");
    }

    @Test
    void lazyRegistrationAndInputRoutesRemainInTheExistingOrder() throws IOException {
        String identity = source("identity/IdentityPanelScreenBootstrap.java");
        assertBefore(identity, "keyBinding();", "ClientTickEvents.END_CLIENT_TICK.register",
            "identity 必须先 lazy register 再挂 tick handler");
        assertTrue(identity.contains("while (keyBinding().wasPressed())"),
            "identity 的按键路由必须继续通过 keyBinding().wasPressed()");
        assertTrue(identity.contains("client.setScreen(new IdentityPanelScreen());"),
            "identity 的屏幕打开行为不能被迁移改动");

        String voidAction = source("cultivation/voidaction/VoidActionScreenBootstrap.java");
        assertBefore(voidAction, "keyBinding();", "ClientTickEvents.END_CLIENT_TICK.register",
            "void action 必须先 lazy register 再挂 tick handler");
        assertTrue(voidAction.contains("while (keyBinding().wasPressed())"),
            "void action 的按键路由必须继续通过 keyBinding().wasPressed()");
        assertTrue(voidAction.contains("client.setScreen(new VoidActionScreen());"),
            "void action 的屏幕打开行为不能被迁移改动");

        String forge = source("forge/ForgeScreenBootstrap.java");
        assertBefore(forge, "keyBinding();", "ClientTickEvents.END_CLIENT_TICK.register",
            "forge 必须先 lazy register 再挂 tick handler");
        assertTrue(forge.contains("while (keyBinding().wasPressed())"),
            "forge 的按键路由必须继续通过 keyBinding().wasPressed()");
        assertTrue(forge.contains("client.setScreen(new ForgeScreen());"),
            "forge 的屏幕打开行为不能被迁移改动");

        String extract = source("tsy/ExtractInteractionBootstrap.java");
        assertBefore(extract, "extractKey = " + REGISTRATION_CALL, "cancelKey = " + REGISTRATION_CALL,
            "extract start 必须保持先于 extract cancel 的注册顺序");
        assertBefore(extract, "cancelKey = " + REGISTRATION_CALL, "ClientTickEvents.END_CLIENT_TICK.register",
            "extract 两个绑定必须在 tick handler 注册前完成");
        assertTrue(extract.contains("while (extractKey.wasPressed() && !ExtractStateStore.snapshot().extracting())"),
            "extract start 的按键路由和 extracting 前置条件必须保持");
        assertTrue(extract.contains("while (cancelKey.wasPressed() && ExtractStateStore.snapshot().extracting())"),
            "extract cancel 的按键路由和 extracting 前置条件必须保持");
        assertTrue(extract.contains("ClientRequestSender.sendStartExtract(portal.entityId());"),
            "extract start 的 C2S 行为必须保持");
        assertTrue(extract.contains("ClientRequestSender.sendCancelExtract();"),
            "extract cancel 的 C2S 行为必须保持");
    }

    private static List<ProductionBinding> productionBindings() throws IOException {
        List<ProductionBinding> result = new java.util.ArrayList<>();
        for (R7SourceScan.ParsedUnit parsed : R7SourceScan.parseJava(CLIENT_ROOT)) {
            String sourcePath = CLIENT_ROOT.relativize(parsed.path()).toString().replace('\\', '/');
            if (!TARGET_FILES.contains(sourcePath)) {
                continue;
            }
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                    Element method = parsed.trees().getElement(getCurrentPath());
                    if (!(method instanceof ExecutableElement executable)
                        || !executable.getSimpleName().contentEquals("register")
                        || !(executable.getEnclosingElement() instanceof TypeElement owner)
                        || !owner.getQualifiedName().contentEquals(BongKeybindRegistry.class.getName())) {
                        return super.visitMethodInvocation(invocation, unused);
                    }

                    assertEquals(1, invocation.getArguments().size(),
                        "global registry register 必须接收一个完整 BindingSpec：" + sourcePath);
                    assertTrue(invocation.getArguments().get(0) instanceof NewClassTree,
                        "global registry register 不得接收拆散或匿名的 binding 参数：" + sourcePath);
                    NewClassTree bindingSpec = (NewClassTree) invocation.getArguments().get(0);
                    TreePath callPath = getCurrentPath();
                    TreePath specPath = new TreePath(callPath, bindingSpec);
                    Element constructor = parsed.trees().getElement(specPath);
                    assertTrue(constructor instanceof ExecutableElement
                            && constructor.getEnclosingElement() instanceof TypeElement specOwner
                            && specOwner.getQualifiedName().contentEquals(
                                "com.bong.client.input.BongKeybindRegistry.BindingSpec"),
                        "global registry 必须使用 BongKeybindRegistry.BindingSpec：" + sourcePath);
                    assertEquals(5, bindingSpec.getArguments().size(),
                        "BindingSpec 必须完整携带 owner/translation/type/default/category：" + sourcePath);

                    List<? extends ExpressionTree> arguments = bindingSpec.getArguments();
                    assertTrue(arguments.get(0) instanceof NewClassTree,
                        "BindingSpec.owner 必须显式构造 BindingOwner：" + sourcePath);
                    NewClassTree bindingOwner = (NewClassTree) arguments.get(0);
                    TreePath ownerPath = new TreePath(specPath, bindingOwner);
                    Element ownerConstructor = parsed.trees().getElement(ownerPath);
                    assertTrue(ownerConstructor instanceof ExecutableElement
                            && ownerConstructor.getEnclosingElement() instanceof TypeElement ownerType
                            && ownerType.getQualifiedName().contentEquals(
                                "com.bong.client.input.BongKeybindRegistry.BindingOwner"),
                        "BindingSpec.owner 必须使用 BongKeybindRegistry.BindingOwner：" + sourcePath);
                    assertEquals(1, bindingOwner.getArguments().size(),
                        "BindingOwner 必须只接受一个 owner id：" + sourcePath);

                    result.add(new ProductionBinding(
                        sourcePath,
                        assignmentTarget(callPath),
                        resolveString(new TreePath(ownerPath, bindingOwner.getArguments().get(0)), parsed),
                        resolveString(new TreePath(specPath, arguments.get(1)), parsed),
                        resolveInputType(new TreePath(specPath, arguments.get(2)), parsed),
                        resolveDefaultContract(new TreePath(specPath, arguments.get(3)), parsed),
                        resolveString(new TreePath(specPath, arguments.get(4)), parsed)
                    ));
                    return super.visitMethodInvocation(invocation, unused);
                }
            }.scan(parsed.unit(), null);
        }
        return result.stream()
            .sorted(java.util.Comparator.comparing(ProductionBinding::sourcePath)
                .thenComparing(ProductionBinding::sourceSite))
            .toList();
    }

    private static String resolveString(TreePath path, R7SourceScan.ParsedUnit parsed) {
        ExpressionTree expression = (ExpressionTree) path.getLeaf();
        if (expression instanceof LiteralTree literal && literal.getValue() instanceof String value) {
            return value;
        }
        Element element = parsed.trees().getElement(path);
        if (element instanceof VariableElement variable && variable.getConstantValue() instanceof String value) {
            return value;
        }
        throw new AssertionError("无法解析字符串常量：" + expression);
    }

    private static String resolveInputType(TreePath path, R7SourceScan.ParsedUnit parsed) {
        Element element = parsed.trees().getElement(path);
        assertTrue(element instanceof VariableElement,
            "InputUtil.Type 必须解析为 enum 常量：" + path.getLeaf());
        VariableElement variable = (VariableElement) element;
        assertTrue(variable.getEnclosingElement() instanceof TypeElement,
            "InputUtil.Type enum 常量必须有类型 owner：" + path.getLeaf());
        assertEquals(InputUtil.Type.class.getCanonicalName(),
            ((TypeElement) variable.getEnclosingElement()).getQualifiedName().toString(),
            "InputUtil.Type 必须是 Minecraft canonical enum：" + path.getLeaf());
        return variable.getSimpleName().toString();
    }

    private static String resolveDefaultContract(TreePath path, R7SourceScan.ParsedUnit parsed) {
        ExpressionTree expression = (ExpressionTree) path.getLeaf();
        if (expression instanceof MethodInvocationTree invocation
            && invocation.getMethodSelect().toString().equals("InputUtil.UNKNOWN_KEY.getCode")) {
            return "UNKNOWN";
        }
        Element element = parsed.trees().getElement(path);
        if (element instanceof VariableElement variable && variable.getConstantValue() instanceof Integer code) {
            if (code == InputUtil.UNKNOWN_KEY.getCode()) {
                return "UNKNOWN";
            }
            if (code >= GLFW.GLFW_KEY_A && code <= GLFW.GLFW_KEY_Z) {
                return Character.toString((char) code.intValue());
            }
        }
        throw new AssertionError("无法解析默认键常量：" + expression);
    }

    private static String assignmentTarget(TreePath path) {
        for (TreePath cursor = path.getParentPath(); cursor != null; cursor = cursor.getParentPath()) {
            if (cursor.getLeaf() instanceof AssignmentTree assignment) {
                return assignment.getVariable().toString().replaceAll("\\s+", " ").trim();
            }
            if (cursor.getLeaf() instanceof VariableTree variable) {
                return variable.getName().toString();
            }
        }
        throw new AssertionError("registry registration 缺少稳定 assignment target：" + path.getLeaf());
    }

    private static List<FixtureRow> fixtureRows() throws IOException {
        try {
            var resource = R7KeybindProductionMigrationTest.class
                .getResource("/bong/ui/keybind-production-sites.tsv");
            assertNotNull(resource, "缺少 R7 production-sites fixture");
            return Files.readAllLines(Path.of(resource.toURI())).stream()
                .filter(R7SourceScan::isFixtureDataLine)
                .map(line -> line.split("\\t", -1))
                .map(columns -> new FixtureRow(
                    columns[0], columns[1], columns[2], columns[3], columns[4],
                    columns[5], columns[6]
                ))
                .toList();
        } catch (URISyntaxException exception) {
            throw new AssertionError("无法定位 R7 production-sites fixture", exception);
        }
    }

    private static String source(String relativePath) {
        return R7SourceScan.read(CLIENT_ROOT.resolve(relativePath));
    }

    private static void assertBefore(String source, String first, String second, String message) {
        int firstIndex = source.indexOf(first);
        int secondIndex = source.indexOf(second);
        assertTrue(firstIndex >= 0, message + "（缺少前置片段：" + first + "）");
        assertTrue(secondIndex >= 0, message + "（缺少后置片段：" + second + "）");
        assertTrue(firstIndex < secondIndex, message);
    }

    private static int occurrences(String source, String token) {
        int count = 0;
        for (int offset = source.indexOf(token); offset >= 0; offset = source.indexOf(token, offset + token.length())) {
            count++;
        }
        return count;
    }

    private static BongKeybindRegistry testRegistry() {
        return new BongKeybindRegistry(
            (UnaryOperator<KeyBinding>) binding -> binding,
            List.of(),
            Set.of()
        );
    }

    private static BongKeybindRegistry.BindingSpec spec(String owner, String translationKey, int defaultCode) {
        return new BongKeybindRegistry.BindingSpec(
            new BongKeybindRegistry.BindingOwner(owner),
            translationKey,
            InputUtil.Type.KEYSYM,
            defaultCode,
            CATEGORY
        );
    }

    private record ProductionBinding(
        String sourcePath,
        String sourceSite,
        String ownerId,
        String translationKey,
        String inputType,
        String defaultContract,
        String category
    ) {
        String siteKey() {
            return sourcePath + "#" + sourceSite;
        }
    }

    private record FixtureRow(
        String ownerId,
        String sourcePath,
        String sourceSite,
        String translationKey,
        String inputType,
        String defaultContract,
        String category
    ) {
        String siteKey() {
            return sourcePath + "#" + sourceSite;
        }

        String normalizedDefaultContract() {
            return defaultContract.startsWith("DEFAULT_KEY=")
                ? defaultContract.substring("DEFAULT_KEY=".length())
                : defaultContract;
        }
    }
}
