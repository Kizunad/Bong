package com.bong.client.network;

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;
import org.junit.jupiter.api.Test;

import javax.lang.model.element.Element;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.VariableElement;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

final class WireS2cContractPinTest {
    private static final String RECEIVER_OWNER =
        "net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking";
    private static final String IDENTIFIER_OWNER = "net.minecraft.util.Identifier";
    private static final Pattern PREFIX_LITERAL = Pattern.compile(
        "\\\"([A-Z][A-Z0-9_]*_)\\\""
    );

    private enum MigrationDecision {
        MIGRATE,
        EXEMPT
    }

    private static final Map<String, MigrationDecision> SIDE_CHANNEL_DECISIONS =
        migrationDecisions();

    private static final Set<String> ENUM_PREFIXES = Set.of(
        "ALCHEMY_OUTCOME_BUCKET_",
        "BOTANY_MODEL_OVERLAY_",
        "CARRIER_CHARGE_PHASE_",
        "CAST_OUTCOME_",
        "CAST_PHASE_",
        "COLOR_KIND_",
        "CONTAINER_KIND_",
        "CRAFT_CATEGORY_",
        "CRAFT_FAILURE_REASON_",
        "DEATH_CINEMATIC_PHASE_",
        "DEATH_CINEMATIC_ZONE_KIND_",
        "DEATH_ROLL_RESULT_",
        "DEATH_SCREEN_STAGE_",
        "DEATH_SCREEN_ZONE_KIND_",
        "EVENT_CHANNEL_",
        "EVENT_KIND_",
        "EVENT_PRIORITY_",
        "EXPOSURE_KIND_",
        "EXTRACT_ABORTED_REASON_",
        "EXTRACT_FAILED_REASON_",
        "FALSE_SKIN_KIND_",
        "FALSE_SKIN_TIER_",
        "FOG_SHAPE_",
        "FORGE_OUTCOME_BUCKET_",
        "FORGE_STEP_",
        "GATHERING_QUALITY_HINT_",
        "GATHERING_TARGET_TYPE_",
        "GUARDIAN_KIND_",
        "INSIGHT_TRIGGER_",
        "KEY_KIND_",
        "LINGTIAN_SESSION_KIND_",
        "MOVEMENT_ACTION_",
        "MOVEMENT_ACTION_REQUEST_KIND_",
        "MOVEMENT_ZONE_KIND_",
        "REALM_",
        "RIFT_PORTAL_DIRECTION_",
        "RIFT_PORTAL_KIND_",
        "SEARCH_ABORT_REASON_",
        "SEASON_",
        "SENSE_KIND_",
        "SKILL_ID_",
        "SPIRIT_TREASURE_DIALOGUE_TONE_",
        "YIDAO_SKILL_ID_"
    );

    @Test
    void everyClientS2cReceiverIsCountedAndEveryBypassHasAMigrationDecision() throws IOException {
        Path sourceRoot = clientRoot().resolve("src/main/java");
        JavaSourceModel sourceModel = JavaSourceModel.parse(sourceRoot);
        List<ReceiverSite> receiverSites = sourceModel.receiverSites();

        assertEquals(32, receiverSites.size(),
            "P0 基线为 32 个真实 Java receiver 调用（server_data 1 + side channels 31）");
        assertEquals(
            List.of(
                Path.of("com/bong/client/BongNetworkHandler.java"),
                Path.of("com/bong/client/iris/IrisBootstrap.java")
            ),
            receiverSites.stream()
                .map(ReceiverSite::relativePath)
                .distinct()
                .sorted(Comparator.comparing(Path::toString))
                .toList(),
            "新增 receiver 文件必须进入 R6 旁路普查，而不是绕过既有 bootstrap"
        );

        Map<String, List<ReceiverSite>> sitesByChannel = new LinkedHashMap<>();
        for (ReceiverSite site : receiverSites) {
            sitesByChannel.computeIfAbsent(site.channel(), ignored -> new ArrayList<>()).add(site);
        }
        List<String> duplicates = sitesByChannel.entrySet().stream()
            .filter(entry -> entry.getValue().size() > 1)
            .map(entry -> entry.getKey() + " -> " + entry.getValue())
            .toList();
        assertEquals(List.of(), duplicates,
            "每个 production receiver channel 必须唯一；重复项需同时报告 call site");
        assertEquals(32, sitesByChannel.size(), "32 个 receiver 必须解析成 32 个唯一 channel ID");
        assertEquals(1, sitesByChannel.getOrDefault("bong:server_data", List.of()).size(),
            "目标轨 bong:server_data 必须且只能注册一次");

        Set<String> actualSideChannels = new TreeSet<>(sitesByChannel.keySet());
        assertTrue(actualSideChannels.remove("bong:server_data"));
        assertEquals(
            SIDE_CHANNEL_DECISIONS.keySet(),
            actualSideChannels,
            "实际解析出的 31 个旁路必须由唯一 migration decision 账本完整覆盖"
        );
        assertEquals(31, SIDE_CHANNEL_DECISIONS.size());
        assertEquals(28, decisionCount(MigrationDecision.MIGRATE));
        assertEquals(3, decisionCount(MigrationDecision.EXEMPT));
        assertEquals(
            Set.of("bong:agent_ui_request", "bong:agent_ui_close", "bong:shader_state"),
            channelsWithDecision(MigrationDecision.EXEMPT),
            "豁免必须是实际旁路的精确子集，不能靠 31-3 的算术掩盖 stale exemption"
        );

        sourceModel.assertReceiverBootstrapsAreReachable();
    }

    @Test
    void protoEnumPrefixInventoryAndNormalizationModesStayFrozen() throws IOException {
        Path networkRoot = clientRoot().resolve("src/main/java/com/bong/client/network");
        String bridgeSource = Files.readString(networkRoot.resolve("ProtoServerDataBridge.java"));
        PrefixInventory bridge = prefixInventory(bridgeSource);

        assertEquals(ENUM_PREFIXES, bridge.prefixes(),
            "ProtoServerDataBridge 新增/删除 enum 前缀必须更新 R6 normalization 账本");
        assertEquals(43, bridge.prefixes().size());
        assertEquals(57, bridge.references(),
            "P0 冻结 bridge-local 的 57 处 enum prefix literal 引用");
        for (String helper : List.of(
            "stripEnumPrefix(",
            "stripEnumPrefixCapitalized(",
            "stripEnumPrefixPascalCase(",
            "bridgeStripEnumsOmittingUnspecified("
        )) {
            assertTrue(bridgeSource.contains(helper),
                () -> "R6 冻结的 enum normalization mode 消失：" + helper);
        }

        String inventorySource = Files.readString(networkRoot.resolve("InventoryEventHandler.java"));
        PrefixInventory handler = prefixInventory(inventorySource);
        assertEquals(Set.of("EQUIP_SLOT_"), handler.prefixes(),
            "P0 当前态只有 InventoryEventHandler 的嵌套 equip.slot 在 bridge 外剥前缀");
        assertEquals(1, handler.references());
        assertTrue(inventorySource.contains("slotName.startsWith(PROTO_EQUIP_SLOT_PREFIX)"));
        assertTrue(inventorySource.contains("slotName.substring(PROTO_EQUIP_SLOT_PREFIX.length())"));

        Set<String> fullReceivePath = new TreeSet<>(bridge.prefixes());
        fullReceivePath.addAll(handler.prefixes());
        assertEquals(44, fullReceivePath.size(),
            "完整 production receive path 基线为 44 个 enum 前缀");
        assertEquals(58, bridge.references() + handler.references(),
            "完整 production receive path 基线为 58 处 enum prefix 引用");
    }

    private static PrefixInventory prefixInventory(String source) {
        Set<String> prefixes = new TreeSet<>();
        Matcher matcher = PREFIX_LITERAL.matcher(source);
        int references = 0;
        while (matcher.find()) {
            prefixes.add(matcher.group(1));
            references++;
        }
        return new PrefixInventory(Set.copyOf(prefixes), references);
    }

    private static Map<String, MigrationDecision> migrationDecisions() {
        Map<String, MigrationDecision> decisions = new LinkedHashMap<>();
        for (String channel : List.of(
            "bong:npc_metadata",
            "bong:npc_lod",
            "bong:npc_bubble",
            "bong:npc_mood",
            "bong:tsy_boss_health",
            "bong:tsy_death_vfx",
            "bong:locust_swarm_warning",
            "bong:vfx_event",
            "bong:vfx/qi_attrition",
            "bong:audio/play",
            "bong:audio/stop",
            "bong:tiandao_presence",
            "bong:audio/ambient_zone",
            "bong:zone_environment",
            "bong:mutation_visual",
            "bong:crack_reading",
            "bong:resonance_lock",
            "bong:resonance_lock_end",
            "bong:void_erosion_visual",
            "bong:spider_disguise_enter",
            "bong:spider_ambush_trigger",
            "bong:rat_qi_tier",
            "bong:daozhan_disguise_enter",
            "bong:daozhan_reveal",
            "bong:core_absorption_hallucination",
            "bong:elder_encounter",
            "bong:era_ambiance",
            "bong:halfstep_rechallenge"
        )) {
            decisions.put(channel, MigrationDecision.MIGRATE);
        }
        decisions.put("bong:agent_ui_request", MigrationDecision.EXEMPT);
        decisions.put("bong:agent_ui_close", MigrationDecision.EXEMPT);
        decisions.put("bong:shader_state", MigrationDecision.EXEMPT);
        return Map.copyOf(decisions);
    }

    private static long decisionCount(MigrationDecision decision) {
        return SIDE_CHANNEL_DECISIONS.values().stream().filter(decision::equals).count();
    }

    private static Set<String> channelsWithDecision(MigrationDecision decision) {
        return SIDE_CHANNEL_DECISIONS.entrySet().stream()
            .filter(entry -> entry.getValue() == decision)
            .map(Map.Entry::getKey)
            .collect(java.util.stream.Collectors.toUnmodifiableSet());
    }

    private record PrefixInventory(Set<String> prefixes, int references) {}

    private record ReceiverSite(
        Path relativePath,
        long line,
        String channel,
        ExecutableElement enclosingMethod
    ) {
        @Override
        public String toString() {
            return relativePath + ":" + line;
        }
    }

    private static final class JavaSourceModel {
        private final Path sourceRoot;
        private final Trees trees;
        private final List<ReceiverSite> receiverSites = new ArrayList<>();
        private final List<String> forbiddenReferences = new ArrayList<>();
        private final Map<ExecutableElement, List<ExecutableElement>> calls = new HashMap<>();
        private final Map<String, ExecutableElement> sourceMethods = new HashMap<>();

        private JavaSourceModel(Path sourceRoot, Trees trees) {
            this.sourceRoot = sourceRoot;
            this.trees = trees;
        }

        static JavaSourceModel parse(Path sourceRoot) throws IOException {
            JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
            assertTrue(compiler != null, "R6 source pin requires a full Java 17 JDK, not a JRE");
            List<Path> sourceFiles;
            try (Stream<Path> files = Files.walk(sourceRoot)) {
                sourceFiles = files
                    .filter(path -> path.toString().endsWith(".java"))
                    .sorted()
                    .toList();
            }

            DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
            try (StandardJavaFileManager fileManager =
                     compiler.getStandardFileManager(diagnostics, null, null)) {
                Iterable<? extends JavaFileObject> javaFiles =
                    fileManager.getJavaFileObjectsFromPaths(sourceFiles);
                List<String> options = List.of(
                    "-proc:none",
                    "--release", "17",
                    "-classpath", System.getProperty("java.class.path")
                );
                JavacTask task = (JavacTask) compiler.getTask(
                    null, fileManager, diagnostics, options, null, javaFiles
                );
                List<CompilationUnitTree> units = new ArrayList<>();
                task.parse().forEach(units::add);
                task.analyze();

                List<String> errors = diagnostics.getDiagnostics().stream()
                    .filter(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR)
                    .filter(diagnostic -> isFatalAttributionDiagnostic(diagnostic, sourceRoot))
                    .map(JavaSourceModel::formatDiagnostic)
                    .toList();
                assertEquals(List.of(), errors,
                    "production Java AST attribution must succeed before receiver census can be trusted");

                JavaSourceModel model = new JavaSourceModel(sourceRoot, Trees.instance(task));
                for (CompilationUnitTree unit : units) {
                    model.scan(unit);
                }
                assertEquals(List.of(), model.forbiddenReferences,
                    "registerGlobalReceiver method references are dynamic registration forms; R6 fails closed");
                return model;
            }
        }

        List<ReceiverSite> receiverSites() {
            return List.copyOf(receiverSites);
        }

        void assertReceiverBootstrapsAreReachable() {
            ExecutableElement initializer = requireSourceMethod(
                "com.bong.client.BongClient", "onInitializeClient", 0
            );
            ExecutableElement networkRegister = requireSourceMethod(
                "com.bong.client.BongNetworkHandler", "register", 0
            );
            ExecutableElement irisRegister = requireSourceMethod(
                "com.bong.client.iris.IrisBootstrap", "register", 0
            );

            assertEquals(1, callCount(initializer, networkRegister),
                "BongClient.onInitializeClient 必须恰好调用一次 BongNetworkHandler.register");
            assertEquals(1, callCount(initializer, irisRegister),
                "BongClient.onInitializeClient 必须恰好调用一次 IrisBootstrap.register");

            Set<ExecutableElement> reachable = reachableFrom(Set.of(networkRegister, irisRegister));
            List<ReceiverSite> unreachable = receiverSites.stream()
                .filter(site -> !reachable.contains(site.enclosingMethod()))
                .toList();
            assertEquals(List.of(), unreachable,
                "每个 receiver 调用都必须从两个 production register bootstrap 可达，不能藏在 dead helper");
        }

        private void scan(CompilationUnitTree unit) {
            new TreePathScanner<Void, ExecutableElement>() {
                @Override
                public Void visitMethod(MethodTree node, ExecutableElement ignored) {
                    Element element = trees.getElement(getCurrentPath());
                    ExecutableElement method = element instanceof ExecutableElement executable
                        ? executable : null;
                    if (method != null) {
                        sourceMethods.put(methodKey(method), method);
                    }
                    return super.visitMethod(node, method);
                }

                @Override
                public Void visitMethodInvocation(
                    MethodInvocationTree node,
                    ExecutableElement enclosingMethod
                ) {
                    Element element = trees.getElement(
                        new TreePath(getCurrentPath(), node.getMethodSelect())
                    );
                    boolean receiverSyntax = isReceiverSyntax(node.getMethodSelect());
                    assertTrue(!receiverSyntax || element instanceof ExecutableElement,
                        () -> "registerGlobalReceiver syntax could not be attributed at " + site(unit, node));
                    assertTrue(!receiverSyntax || isReceiverMethod((ExecutableElement) element),
                        () -> "registerGlobalReceiver must resolve to Fabric's exact owner at "
                            + site(unit, node));
                    if (enclosingMethod != null && element instanceof ExecutableElement called) {
                        calls.computeIfAbsent(enclosingMethod, ignored -> new ArrayList<>()).add(called);
                    }
                    if (element instanceof ExecutableElement called && isReceiverMethod(called)) {
                        assertTrue(enclosingMethod != null,
                            "receiver invocation must be enclosed by a production method");
                        assertTrue(!node.getArguments().isEmpty(),
                            "registerGlobalReceiver must have an Identifier first argument");
                        TreePath argumentPath = new TreePath(getCurrentPath(), node.getArguments().get(0));
                        String channel = resolveIdentifier(argumentPath, new HashSet<>());
                        assertTrue(channel != null,
                            () -> "receiver channel must be statically resolvable at " + site(unit, node));
                        receiverSites.add(new ReceiverSite(
                            relativePath(unit),
                            line(unit, node),
                            channel,
                            enclosingMethod
                        ));
                    }
                    return super.visitMethodInvocation(node, enclosingMethod);
                }

                @Override
                public Void visitMemberReference(
                    MemberReferenceTree node,
                    ExecutableElement enclosingMethod
                ) {
                    Element element = trees.getElement(getCurrentPath());
                    if (element instanceof ExecutableElement called && isReceiverMethod(called)) {
                        forbiddenReferences.add(site(unit, node));
                    }
                    return super.visitMemberReference(node, enclosingMethod);
                }
            }.scan(unit, null);
        }

        private String resolveIdentifier(TreePath path, Set<Element> resolving) {
            Tree leaf = path.getLeaf();
            if (leaf instanceof ParenthesizedTree parenthesized) {
                return resolveIdentifier(new TreePath(path, parenthesized.getExpression()), resolving);
            }
            if (leaf instanceof NewClassTree constructor) {
                Element type = trees.getElement(new TreePath(path, constructor.getIdentifier()));
                if (!(type instanceof TypeElement typeElement)
                    || !IDENTIFIER_OWNER.contentEquals(typeElement.getQualifiedName())
                    || constructor.getArguments().size() != 2) {
                    return null;
                }
                String namespace = resolveString(
                    new TreePath(path, constructor.getArguments().get(0)), resolving
                );
                String channelPath = resolveString(
                    new TreePath(path, constructor.getArguments().get(1)), resolving
                );
                return namespace == null || channelPath == null
                    ? null : namespace + ":" + channelPath;
            }

            Element element = trees.getElement(path);
            if (!(element instanceof VariableElement variable) || !resolving.add(variable)) {
                return null;
            }
            try {
                TreePath declarationPath = trees.getPath(variable);
                if (declarationPath == null || !(declarationPath.getLeaf() instanceof VariableTree declaration)
                    || declaration.getInitializer() == null) {
                    return null;
                }
                return resolveIdentifier(
                    new TreePath(declarationPath, declaration.getInitializer()), resolving
                );
            } finally {
                resolving.remove(variable);
            }
        }

        private String resolveString(TreePath path, Set<Element> resolving) {
            Tree leaf = path.getLeaf();
            if (leaf instanceof ParenthesizedTree parenthesized) {
                return resolveString(new TreePath(path, parenthesized.getExpression()), resolving);
            }
            if (leaf instanceof LiteralTree literal && literal.getValue() instanceof String value) {
                return value;
            }
            Element element = trees.getElement(path);
            if (element instanceof VariableElement variable
                && variable.getConstantValue() instanceof String value) {
                return value;
            }
            if (!(element instanceof VariableElement variable) || !resolving.add(variable)) {
                return null;
            }
            try {
                TreePath declarationPath = trees.getPath(variable);
                if (declarationPath == null || !(declarationPath.getLeaf() instanceof VariableTree declaration)
                    || declaration.getInitializer() == null) {
                    return null;
                }
                return resolveString(new TreePath(declarationPath, declaration.getInitializer()), resolving);
            } finally {
                resolving.remove(variable);
            }
        }

        private ExecutableElement requireSourceMethod(String owner, String name, int parameterCount) {
            String key = owner + "#" + name + "/" + parameterCount;
            ExecutableElement method = sourceMethods.get(key);
            if (method == null) {
                fail("production bootstrap method disappeared: " + key);
            }
            return method;
        }

        private int callCount(ExecutableElement caller, ExecutableElement callee) {
            return (int) calls.getOrDefault(caller, List.of()).stream()
                .filter(callee::equals)
                .count();
        }

        private Set<ExecutableElement> reachableFrom(Set<ExecutableElement> roots) {
            Set<ExecutableElement> reachable = new HashSet<>();
            ArrayDeque<ExecutableElement> pending = new ArrayDeque<>(roots);
            while (!pending.isEmpty()) {
                ExecutableElement method = pending.removeFirst();
                if (!reachable.add(method)) {
                    continue;
                }
                for (ExecutableElement called : calls.getOrDefault(method, List.of())) {
                    if (sourceMethods.containsValue(called)) {
                        pending.addLast(called);
                    }
                }
            }
            return reachable;
        }

        private boolean isReceiverMethod(ExecutableElement method) {
            return method.getSimpleName().contentEquals("registerGlobalReceiver")
                && method.getEnclosingElement() instanceof TypeElement owner
                && RECEIVER_OWNER.contentEquals(owner.getQualifiedName());
        }

        private static boolean isReceiverSyntax(ExpressionTree methodSelect) {
            if (methodSelect instanceof MemberSelectTree memberSelect) {
                return memberSelect.getIdentifier().contentEquals("registerGlobalReceiver");
            }
            return methodSelect instanceof IdentifierTree identifier
                && identifier.getName().contentEquals("registerGlobalReceiver");
        }

        private static boolean isFatalAttributionDiagnostic(
            Diagnostic<? extends JavaFileObject> diagnostic,
            Path sourceRoot
        ) {
            JavaFileObject source = diagnostic.getSource();
            if (source == null) {
                return true;
            }
            Path sourcePath = Path.of(source.toUri()).normalize();
            if (!sourcePath.startsWith(sourceRoot)) {
                return true;
            }
            Path relative = sourceRoot.relativize(sourcePath);
            return relative.equals(Path.of("com/bong/client/BongClient.java"))
                || relative.equals(Path.of("com/bong/client/BongNetworkHandler.java"))
                || relative.equals(Path.of("com/bong/client/iris/IrisBootstrap.java"));
        }

        private Path relativePath(CompilationUnitTree unit) {
            return sourceRoot.relativize(Path.of(unit.getSourceFile().toUri()));
        }

        private long line(CompilationUnitTree unit, Tree tree) {
            long position = trees.getSourcePositions().getStartPosition(unit, tree);
            return unit.getLineMap().getLineNumber(position);
        }

        private String site(CompilationUnitTree unit, Tree tree) {
            return relativePath(unit) + ":" + line(unit, tree);
        }

        private static String methodKey(ExecutableElement method) {
            Element owner = method.getEnclosingElement();
            String ownerName = owner instanceof TypeElement type
                ? type.getQualifiedName().toString() : owner.toString();
            return ownerName + "#" + method.getSimpleName() + "/" + method.getParameters().size();
        }

        private static String formatDiagnostic(Diagnostic<? extends JavaFileObject> diagnostic) {
            JavaFileObject source = diagnostic.getSource();
            String location = source == null ? "<unknown>" : source.getName();
            return location + ":" + diagnostic.getLineNumber() + ": "
                + diagnostic.getMessage(null);
        }
    }

    private static Path clientRoot() {
        Path candidate = Path.of("").toAbsolutePath().normalize();
        while (candidate != null) {
            if (Files.isDirectory(candidate.resolve("src/main/java/com/bong/client"))) {
                return candidate;
            }
            Path nestedClient = candidate.resolve("client");
            if (Files.isDirectory(nestedClient.resolve("src/main/java/com/bong/client"))) {
                return nestedClient;
            }
            candidate = candidate.getParent();
        }
        throw new AssertionError("无法定位 client source tree");
    }
}
