package com.bong.client.ui.bootstrap;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * 显式启动注册的确定性依赖图。每个成功模块都单独记账，因此失败
 * 后重试时只会重跑失败节点，不会重复早先已经完成的 Fabric 注册。
 */
public final class UiBootstrapRegistry {
    private final Map<String, UiBootstrapModule> modules = new LinkedHashMap<>();
    private final Set<String> completed = new LinkedHashSet<>();
    private boolean registrationStarted;
    private UiRuntime registeredRuntime;

    public void add(UiBootstrapModule module) {
        Objects.requireNonNull(module, "module must not be null");
        if (registrationStarted) {
            throw new IllegalStateException("cannot add a bootstrap module after registration started");
        }
        String id = requireId(module.id());
        if (modules.containsKey(id)) {
            throw new IllegalArgumentException("duplicate bootstrap module id: " + id);
        }
        Set<String> dependencies = module.dependencies();
        if (dependencies == null) {
            throw new IllegalArgumentException("dependencies must not be null for " + id);
        }
        for (String dependency : dependencies) {
            requireId(dependency);
        }
        modules.put(id, new CheckedModule(module, Set.copyOf(dependencies)));
    }

    public List<String> registrationOrder() {
        List<String> order = new ArrayList<>();
        Set<String> visiting = new LinkedHashSet<>();
        Set<String> visited = new LinkedHashSet<>();
        for (String id : modules.keySet()) {
            visit(id, visiting, visited, order);
        }
        return List.copyOf(order);
    }

    public void registerAll(UiRuntime runtime) {
        beginRegistration(runtime);
        for (String id : registrationOrder()) {
            registerChecked(id, runtime);
        }
    }

    /** 注册一个模块及其依赖，供生产 bootstrap 按原有时序逐批迁移。 */
    public void register(String id, UiRuntime runtime) {
        beginRegistration(runtime);
        String checkedId = requireId(id);
        List<String> order = new ArrayList<>();
        visit(checkedId, new LinkedHashSet<>(), new LinkedHashSet<>(), order);
        for (String dependencyOrTarget : order) {
            registerChecked(dependencyOrTarget, runtime);
        }
    }

    public List<String> moduleIds() {
        return List.copyOf(modules.keySet());
    }

    public List<String> completedModuleIds() {
        return List.copyOf(completed);
    }

    public boolean isRegistered(String id) {
        return completed.contains(id);
    }

    private void beginRegistration(UiRuntime runtime) {
        Objects.requireNonNull(runtime, "runtime must not be null");
        if (registeredRuntime != null && registeredRuntime != runtime) {
            throw new IllegalStateException("bootstrap registry cannot switch runtime after registration started");
        }
        registeredRuntime = runtime;
        registrationStarted = true;
    }

    private void registerChecked(String id, UiRuntime runtime) {
        if (completed.contains(id)) {
            return;
        }
        modules.get(id).register(runtime);
        completed.add(id);
    }

    private void visit(String id, Set<String> visiting, Set<String> visited, List<String> order) {
        if (visited.contains(id)) {
            return;
        }
        if (!visiting.add(id)) {
            throw new IllegalArgumentException("bootstrap dependency cycle at: " + id);
        }
        UiBootstrapModule module = modules.get(id);
        if (module == null) {
            throw new IllegalArgumentException("missing bootstrap dependency: " + id);
        }
        List<String> dependencies = new ArrayList<>(module.dependencies());
        Collections.sort(dependencies);
        for (String dependency : dependencies) {
            visit(dependency, visiting, visited, order);
        }
        visiting.remove(id);
        visited.add(id);
        order.add(id);
    }

    private static String requireId(String id) {
        Objects.requireNonNull(id, "module id must not be null");
        if (id.isBlank()) {
            throw new IllegalArgumentException("module id must not be blank");
        }
        return id;
    }

    private record CheckedModule(UiBootstrapModule delegate, Set<String> dependencies)
        implements UiBootstrapModule {
        @Override
        public String id() {
            return delegate.id();
        }

        @Override
        public void register(UiRuntime runtime) {
            delegate.register(runtime);
        }
    }
}
