package com.bong.client.ui;

import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.UnaryOperator;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BongKeybindRegistryTest {
    private static final String CATEGORY = "category.test.controls";

    @Test
    void globalIsTheSingletonProductionRegistry() {
        assertSame(BongKeybindRegistry.global(), BongKeybindRegistry.global(),
            "global() must return one shared registry so bootstrap-local gates cannot diverge");
    }

    @Test
    void duplicateOwnerIdAndTranslationKeyAreRejectedBeforeRegistrarRuns() {
        AtomicInteger calls = new AtomicInteger();
        BongKeybindRegistry registry = registry(
            binding -> {
                calls.incrementAndGet();
                return binding;
            }, List.of(), Set.of()
        );

        registry.register(spec("owner.one", "key.test.first", GLFW.GLFW_KEY_A));

        IllegalArgumentException duplicateOwner = assertThrows(IllegalArgumentException.class,
            () -> registry.register(spec("owner.one", "key.test.second", GLFW.GLFW_KEY_B)));
        assertTrue(duplicateOwner.getMessage().contains("owner.one"),
            "duplicate owner failure should identify the rejected owner");

        IllegalArgumentException duplicateTranslation = assertThrows(IllegalArgumentException.class,
            () -> registry.register(spec("owner.two", "key.test.first", GLFW.GLFW_KEY_C)));
        assertTrue(duplicateTranslation.getMessage().contains("key.test.first"),
            "duplicate translation failure should identify the rejected translation key");
        assertEquals(1, calls.get(), "rejected identity checks must not call the registrar");
    }

    @Test
    void physicalConflictUsesTypeAndCodeAsTheExactIdentity() {
        BongKeybindRegistry registry = registry();
        registry.register(spec("key.keysym", "key.test.keysym", InputUtil.Type.KEYSYM, 17));

        assertThrows(IllegalArgumentException.class,
            () -> registry.register(spec("key.keysym.duplicate", "key.test.duplicate", InputUtil.Type.KEYSYM, 17)),
            "same InputUtil.Type and code must be a physical conflict");

        BongKeybindRegistry differentType = registry();
        differentType.register(spec("key.keysym", "key.test.keysym", InputUtil.Type.KEYSYM, 17));
        differentType.register(spec("key.scancode", "key.test.scancode", InputUtil.Type.SCANCODE, 17));
        assertEquals(2, differentType.registrations().size(),
            "the same numeric code under a different InputUtil.Type is not a conflict");
    }

    @Test
    void unknownDefaultDoesNotParticipateInPhysicalConflictDetection() {
        BongKeybindRegistry registry = registry();
        int unknown = InputUtil.UNKNOWN_KEY.getCode();

        registry.register(spec("unknown.one", "key.test.unknown.one", InputUtil.Type.KEYSYM, unknown));
        registry.register(spec("unknown.two", "key.test.unknown.two", InputUtil.Type.KEYSYM, unknown));

        assertEquals(2, registry.registrations().size(),
            "unbound defaults may repeat because UNKNOWN has no physical key identity");
    }

    @Test
    void globalVanillaReservationsProtectChatAndAdvancementsDefaults() {
        BongKeybindRegistry registry = BongKeybindRegistry.global();
        assertThrows(IllegalArgumentException.class,
            () -> registry.register(spec("bong.chat", "key.test.chat", GLFW.GLFW_KEY_T)),
            "KEYSYM+T is reserved by vanilla chat");
        assertThrows(IllegalArgumentException.class,
            () -> registry.register(spec("bong.advancements", "key.test.advancements", GLFW.GLFW_KEY_L)),
            "KEYSYM+L is reserved by vanilla advancements");
        assertSame(registry, BongKeybindRegistry.global(),
            "reservation verification must not replace the singleton");
    }

    @Test
    void onlyExactOwnerPairAndPhysicalKeyWithReasonCanExemptConflict() {
        BongKeybindRegistry.BindingOwner first = new BongKeybindRegistry.BindingOwner("owner.first");
        BongKeybindRegistry.BindingOwner second = new BongKeybindRegistry.BindingOwner("owner.second");
        BongKeybindRegistry.PhysicalDefault key = new BongKeybindRegistry.PhysicalDefault(
            InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_Q
        );
        BongKeybindRegistry registry = registry(
            binding -> binding,
            List.of(),
            Set.of(new BongKeybindRegistry.ConflictExemption(first, second, key, "intentional chord"))
        );

        registry.register(new BongKeybindRegistry.BindingSpec(
            first, "key.test.first", key.type(), key.code(), CATEGORY
        ));
        registry.register(new BongKeybindRegistry.BindingSpec(
            second, "key.test.second", key.type(), key.code(), CATEGORY
        ));

        BongKeybindRegistry wrongOwner = registry(
            binding -> binding,
            List.of(),
            Set.of(new BongKeybindRegistry.ConflictExemption(first, second, key, "intentional chord"))
        );
        wrongOwner.register(new BongKeybindRegistry.BindingSpec(
            first, "key.test.first", key.type(), key.code(), CATEGORY
        ));
        assertThrows(IllegalArgumentException.class,
            () -> wrongOwner.register(new BongKeybindRegistry.BindingSpec(
                new BongKeybindRegistry.BindingOwner("owner.third"),
                "key.test.third", key.type(), key.code(), CATEGORY
            )),
            "an exemption for owner.first/owner.second cannot authorize owner.third");

        BongKeybindRegistry wrongKey = registry(
            binding -> binding,
            List.of(),
            Set.of(new BongKeybindRegistry.ConflictExemption(
                first, second,
                new BongKeybindRegistry.PhysicalDefault(InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_W),
                "different physical key"
            ))
        );
        wrongKey.register(new BongKeybindRegistry.BindingSpec(
            first, "key.test.first", key.type(), key.code(), CATEGORY
        ));
        assertThrows(IllegalArgumentException.class,
            () -> wrongKey.register(new BongKeybindRegistry.BindingSpec(
                second, "key.test.second", key.type(), key.code(), CATEGORY
            )),
            "an exemption for another physical key cannot authorize this collision");
    }

    @Test
    void reservationConflictCanUseOnlyAnExactNonBlankExemption() {
        BongKeybindRegistry.BindingOwner vanilla = new BongKeybindRegistry.BindingOwner("vanilla.chat");
        BongKeybindRegistry.BindingOwner owner = new BongKeybindRegistry.BindingOwner("owner.chat_overlay");
        BongKeybindRegistry.PhysicalDefault key = new BongKeybindRegistry.PhysicalDefault(
            InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_T
        );

        BongKeybindRegistry registry = registry(
            binding -> binding,
            List.of(new BongKeybindRegistry.ReservedDefault(vanilla, key)),
            Set.of(new BongKeybindRegistry.ConflictExemption(
                owner, vanilla, key, "overlay intentionally shares chat key"
            ))
        );
        registry.register(new BongKeybindRegistry.BindingSpec(
            owner, "key.test.chat_overlay", key.type(), key.code(), CATEGORY
        ));
        assertEquals(1, registry.registrations().size(),
            "the exact owner pair and exact reserved key should authorize this one conflict");

        assertThrows(IllegalArgumentException.class,
            () -> new BongKeybindRegistry.ConflictExemption(owner, vanilla, key, "   "),
            "an exemption reason containing only whitespace is not actionable");
    }

    @Test
    void registrationsPreserveSuccessfulOrderAndCannotBeMutated() {
        BongKeybindRegistry registry = registry();
        KeyBinding first = registry.register(spec("owner.first", "key.test.first", GLFW.GLFW_KEY_A));
        KeyBinding second = registry.register(spec("owner.second", "key.test.second", GLFW.GLFW_KEY_B));

        List<BongKeybindRegistry.Registration> registrations = registry.registrations();
        assertEquals(List.of(first, second), registrations.stream()
            .map(BongKeybindRegistry.Registration::binding).toList(),
            "registrations must remain in successful registration order");
        assertEquals("owner.first", registrations.get(0).owner().id(),
            "the first registration must retain its owner identity");
        assertThrows(UnsupportedOperationException.class,
            () -> registrations.add(registrations.get(0)),
            "registrations() must return an immutable snapshot");

        registry.register(spec("owner.third", "key.test.third", GLFW.GLFW_KEY_C));
        assertEquals(2, registrations.size(),
            "an earlier returned snapshot must not be mutated by later registration");
        assertEquals(3, registry.registrations().size(),
            "a later snapshot must observe the new successful registration");
    }

    @Test
    void legacyBindingMigratesByTranslationKeyAndUpdatesPhysicalIndex() {
        BongKeybindRegistry registry = registry();
        KeyBinding unrelated = registry.register(spec(
            "owner.unrelated", "key.test.unrelated", GLFW.GLFW_KEY_K
        ));
        KeyBinding forge = registry.register(spec(
            "owner.forge", "key.test.forge", GLFW.GLFW_KEY_U
        ));
        AtomicReference<InputUtil.Key> persisted = new AtomicReference<>();

        KeyBinding.unpressAll();
        KeyBinding.onKeyPressed(key(GLFW.GLFW_KEY_U));
        assertTrue(forge.wasPressed(),
            "before migration the persisted legacy U must still reach Forge's binding");
        assertFalse(forge.wasPressed(), "the pre-migration press must be drained before migration");

        boolean migrated = registry.migrateLegacyBoundKey(
            "key.test.forge",
            key(GLFW.GLFW_KEY_U),
            InputUtil.UNKNOWN_KEY,
            (binding, replacement) -> {
                assertSame(forge, binding,
                    "migration must select the binding by translation key, not registration order");
                binding.setBoundKey(replacement);
                persisted.set(replacement);
            }
        );

        assertTrue(migrated, "an exact legacy U binding must be migrated");
        assertTrue(forge.isUnbound(), "legacy Forge binding must become unbound");
        assertEquals(InputUtil.UNKNOWN_KEY, persisted.get(),
            "migration must pass UNKNOWN to the persistence seam");
        assertTrue(unrelated.matchesKey(GLFW.GLFW_KEY_K, 0),
            "migration must not alter another registered binding");

        KeyBinding.onKeyPressed(key(GLFW.GLFW_KEY_U));
        assertFalse(forge.wasPressed(),
            "after migration the old U physical index must no longer dispatch to Forge");
    }

    @Test
    void customizedBindingIsPreservedAndRebinderIsNotCalled() {
        BongKeybindRegistry registry = registry();
        KeyBinding forge = registry.register(spec(
            "owner.forge.custom", "key.test.forge.custom", GLFW.GLFW_KEY_K
        ));
        AtomicInteger rebinderCalls = new AtomicInteger();

        boolean migrated = registry.migrateLegacyBoundKey(
            "key.test.forge.custom",
            key(GLFW.GLFW_KEY_U),
            InputUtil.UNKNOWN_KEY,
            (binding, replacement) -> rebinderCalls.incrementAndGet()
        );

        assertFalse(migrated, "a player-customized K binding is not a legacy U binding");
        assertEquals(0, rebinderCalls.get(),
            "customized bindings must not reach the persistence seam");
        assertTrue(forge.matchesKey(GLFW.GLFW_KEY_K, 0),
            "player-customized key must remain unchanged");
    }

    @Test
    void alreadyUnboundBindingIsIdempotentlyLeftUnchanged() {
        BongKeybindRegistry registry = registry();
        KeyBinding forge = registry.register(spec(
            "owner.forge.unbound", "key.test.forge.unbound", InputUtil.UNKNOWN_KEY.getCode()
        ));
        AtomicInteger rebinderCalls = new AtomicInteger();

        boolean migrated = registry.migrateLegacyBoundKey(
            "key.test.forge.unbound",
            key(GLFW.GLFW_KEY_U),
            InputUtil.UNKNOWN_KEY,
            (binding, replacement) -> rebinderCalls.incrementAndGet()
        );

        assertFalse(migrated, "an already UNKNOWN binding has no legacy value to migrate");
        assertEquals(0, rebinderCalls.get(),
            "idempotent no-op must not rewrite options.txt");
        assertTrue(forge.isUnbound(), "already UNKNOWN must remain UNKNOWN");
    }

    @Test
    void nullAndBlankInputsFailFastAtTheContractBoundary() {
        assertThrows(NullPointerException.class, () -> new BongKeybindRegistry.BindingOwner(null));
        assertThrows(IllegalArgumentException.class, () -> new BongKeybindRegistry.BindingOwner("\t"));
        assertThrows(NullPointerException.class, () -> new BongKeybindRegistry.PhysicalDefault(null, 1));

        BongKeybindRegistry.BindingOwner owner = new BongKeybindRegistry.BindingOwner("owner.valid");
        assertThrows(NullPointerException.class,
            () -> new BongKeybindRegistry.BindingSpec(null, "key.valid", InputUtil.Type.KEYSYM, 1, CATEGORY));
        assertThrows(IllegalArgumentException.class,
            () -> new BongKeybindRegistry.BindingSpec(owner, " ", InputUtil.Type.KEYSYM, 1, CATEGORY));
        assertThrows(NullPointerException.class,
            () -> new BongKeybindRegistry.BindingSpec(owner, "key.valid", null, 1, CATEGORY));
        assertThrows(IllegalArgumentException.class,
            () -> new BongKeybindRegistry.BindingSpec(owner, "key.valid", InputUtil.Type.KEYSYM, 1, " "));

        assertThrows(NullPointerException.class,
            () -> new BongKeybindRegistry(null, List.of(), Set.of()));
        assertThrows(NullPointerException.class,
            () -> new BongKeybindRegistry(binding -> binding, null, Set.of()));
        assertThrows(NullPointerException.class,
            () -> new BongKeybindRegistry(binding -> binding, List.of(), null));
        assertThrows(NullPointerException.class,
            () -> new BongKeybindRegistry(binding -> binding, List.of((BongKeybindRegistry.ReservedDefault) null), Set.of()));

        BongKeybindRegistry registry = registry();
        assertThrows(NullPointerException.class, () -> registry.register(null));
    }

    private static BongKeybindRegistry registry() {
        return registry(binding -> binding, List.of(), Set.of());
    }

    private static BongKeybindRegistry registry(
        UnaryOperator<KeyBinding> registrar,
        List<BongKeybindRegistry.ReservedDefault> reservedDefaults,
        Set<BongKeybindRegistry.ConflictExemption> exemptions
    ) {
        return new BongKeybindRegistry(registrar, reservedDefaults, exemptions);
    }

    private static BongKeybindRegistry.BindingSpec spec(String owner, String translationKey, int code) {
        return spec(owner, translationKey, InputUtil.Type.KEYSYM, code);
    }

    private static BongKeybindRegistry.BindingSpec spec(
        String owner,
        String translationKey,
        InputUtil.Type type,
        int code
    ) {
        return new BongKeybindRegistry.BindingSpec(
            new BongKeybindRegistry.BindingOwner(owner), translationKey, type, code, CATEGORY
        );
    }

    private static InputUtil.Key key(int code) {
        return InputUtil.Type.KEYSYM.createFromCode(code);
    }

}
