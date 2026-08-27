package com.bong.client.ui.bootstrap;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class UiBootstrapRegistryTest {
    @Test
    void topologyIsDeterministicAndRepeatedRegistrationIsIdempotent() {
        UiBootstrapRegistry registry = new UiBootstrapRegistry();
        List<String> calls = new ArrayList<>();
        registry.add(module("screen", Set.of("network"), calls));
        registry.add(module("network", Set.of(), calls));
        registry.add(module("hud", Set.of("screen"), calls));
        assertEquals(List.of("network", "screen", "hud"), registry.registrationOrder());

        UiRuntime runtime = new UiRuntime() {
        };
        registry.registerAll(runtime);
        registry.registerAll(runtime);
        assertEquals(List.of("network", "screen", "hud"), calls,
            "重复 registerAll 不能重复注册 Fabric callback 或 HUD");
        assertEquals(List.of("network", "screen", "hud"), registry.completedModuleIds());
    }

    @Test
    void missingDependencyCycleDuplicateAndLateModuleAreRejected() {
        UiBootstrapRegistry missing = new UiBootstrapRegistry();
        missing.add(module("screen", Set.of("missing"), new ArrayList<>()));
        assertThrows(IllegalArgumentException.class, missing::registrationOrder);

        UiBootstrapRegistry cycle = new UiBootstrapRegistry();
        cycle.add(module("a", Set.of("b"), new ArrayList<>()));
        cycle.add(module("b", Set.of("a"), new ArrayList<>()));
        assertThrows(IllegalArgumentException.class, cycle::registrationOrder);

        UiBootstrapRegistry duplicate = new UiBootstrapRegistry();
        duplicate.add(module("same", Set.of(), new ArrayList<>()));
        assertThrows(IllegalArgumentException.class,
            () -> duplicate.add(module("same", Set.of(), new ArrayList<>())));

        duplicate.registerAll(new UiRuntime() {
        });
        assertThrows(IllegalStateException.class,
            () -> duplicate.add(module("late", Set.of(), new ArrayList<>())));
    }

    @Test
    void failedModuleCanRetryWithoutRepeatingSuccessfulDependencies() {
        UiBootstrapRegistry registry = new UiBootstrapRegistry();
        AtomicInteger dependencyCalls = new AtomicInteger();
        AtomicInteger failedCalls = new AtomicInteger();
        AtomicBoolean failOnce = new AtomicBoolean(true);
        registry.add(new UiBootstrapModule() {
            @Override
            public String id() {
                return "dependency";
            }

            @Override
            public Set<String> dependencies() {
                return Set.of();
            }

            @Override
            public void register(UiRuntime runtime) {
                dependencyCalls.incrementAndGet();
            }
        });
        registry.add(new UiBootstrapModule() {
            @Override
            public String id() {
                return "failing";
            }

            @Override
            public Set<String> dependencies() {
                return Set.of("dependency");
            }

            @Override
            public void register(UiRuntime runtime) {
                failedCalls.incrementAndGet();
                if (failOnce.getAndSet(false)) {
                    throw new IllegalStateException("bootstrap failed");
                }
            }
        });
        UiRuntime runtime = new UiRuntime() {
        };
        assertThrows(IllegalStateException.class, () -> registry.registerAll(runtime));
        registry.registerAll(runtime);
        assertEquals(1, dependencyCalls.get());
        assertEquals(2, failedCalls.get());
    }

    @Test
    void runtimeCannotBeSwappedAfterRegistrationStarts() {
        UiBootstrapRegistry registry = new UiBootstrapRegistry();
        registry.add(module("one", Set.of(), new ArrayList<>()));
        registry.registerAll(new UiRuntime() {
        });
        assertThrows(IllegalStateException.class, () -> registry.registerAll(new UiRuntime() {
        }));
    }

    @Test
    void targetedRegistrationRunsOnlyTheRequestedDependencyClosure() {
        UiBootstrapRegistry registry = new UiBootstrapRegistry();
        List<String> calls = new ArrayList<>();
        registry.add(module("root", Set.of(), calls));
        registry.add(module("requested", Set.of("root"), calls));
        registry.add(module("unrelated", Set.of(), calls));
        UiRuntime runtime = new UiRuntime() {
        };

        registry.register("requested", runtime);
        registry.register("requested", runtime);

        assertEquals(List.of("root", "requested"), calls);
        assertEquals(List.of("root", "requested"), registry.completedModuleIds());
    }

    private static UiBootstrapModule module(String id, Set<String> dependencies, List<String> calls) {
        return new UiBootstrapModule() {
            @Override
            public String id() {
                return id;
            }

            @Override
            public Set<String> dependencies() {
                return dependencies;
            }

            @Override
            public void register(UiRuntime runtime) {
                calls.add(id);
            }
        };
    }
}
