package com.bong.client.ui;

import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.function.BiConsumer;
import java.util.function.UnaryOperator;

/**
 * Explicit keybinding registration gate for Bong client bindings.
 *
 * <p>The registry owns logical identity and default-key validation.  It does
 * not discover bindings: each caller must pass a complete {@link BindingSpec}
 * to {@link #register(BindingSpec)}.</p>
 */
public final class BongKeybindRegistry {
    private static final KeyBindingFactory KEY_BINDING_FACTORY = KeyBinding::new;
    private static final BongKeybindRegistry GLOBAL = new BongKeybindRegistry(
        KeyBindingHelper::registerKeyBinding,
        List.of(
            new ReservedDefault(
                new BindingOwner("vanilla.chat"),
                new PhysicalDefault(InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_T)
            ),
            new ReservedDefault(
                new BindingOwner("vanilla.advancements"),
                new PhysicalDefault(InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_L)
            )
        ),
        Set.of()
    );

    private final UnaryOperator<KeyBinding> registrar;
    private final List<ReservedDefault> reservedDefaults;
    private final Set<ConflictExemption> exemptions;
    private final Set<String> reservedOwnerIds;
    private final Set<String> ownerIds = new HashSet<>();
    private final Set<String> translationKeys = new HashSet<>();
    private final List<Registration> registrations = new ArrayList<>();

    @FunctionalInterface
    private interface KeyBindingFactory {
        KeyBinding create(String translationKey, InputUtil.Type type, int defaultCode, String category);
    }

    /** Package-private injection seam used by behavior tests and client bootstrap wiring. */
    BongKeybindRegistry(
        UnaryOperator<KeyBinding> registrar,
        List<ReservedDefault> reservedDefaults,
        Set<ConflictExemption> exemptions
    ) {
        this.registrar = Objects.requireNonNull(registrar, "registrar must not be null");
        this.reservedDefaults = List.copyOf(
            Objects.requireNonNull(reservedDefaults, "reservedDefaults must not be null")
        );
        this.exemptions = Set.copyOf(
            Objects.requireNonNull(exemptions, "exemptions must not be null")
        );
        this.reservedOwnerIds = this.reservedDefaults.stream()
            .map(reserved -> reserved.owner().id())
            .collect(java.util.stream.Collectors.toUnmodifiableSet());
    }

    /** Returns the one production registry shared by all client bootstrap owners. */
    public static BongKeybindRegistry global() {
        return GLOBAL;
    }

    /**
     * Validates and registers one explicit binding.
     *
     * <p>All validation happens before the injected registrar is called.  A
     * failed registration therefore cannot consume a Fabric registration slot
     * or alter the observable registration sequence.</p>
     */
    public synchronized KeyBinding register(BindingSpec spec) {
        Objects.requireNonNull(spec, "spec must not be null");

        String ownerId = spec.owner().id();
        if (reservedOwnerIds.contains(ownerId) || ownerIds.contains(ownerId)) {
            throw new IllegalArgumentException("duplicate binding owner id: " + ownerId);
        }
        if (translationKeys.contains(spec.translationKey())) {
            throw new IllegalArgumentException(
                "duplicate binding translation key: " + spec.translationKey()
            );
        }

        PhysicalDefault key = new PhysicalDefault(spec.type(), spec.defaultCode());
        if (participatesInPhysicalConflict(key)) {
            rejectReservedConflicts(spec.owner(), key);
            rejectRegistrationConflicts(spec.owner(), key);
        }

        KeyBinding binding = Objects.requireNonNull(
            registrar.apply(KEY_BINDING_FACTORY.create(
                spec.translationKey(), spec.type(), spec.defaultCode(), spec.category()
            )),
            "registrar must return the created KeyBinding"
        );
        Registration registration = new Registration(spec.owner(), spec, binding);
        ownerIds.add(ownerId);
        translationKeys.add(spec.translationKey());
        registrations.add(registration);
        return binding;
    }

    /** Returns successful registrations in registration order, without mutable ownership. */
    public synchronized List<Registration> registrations() {
        return List.copyOf(registrations);
    }

    /**
     * Migrates one persisted binding only when it still has the exact legacy
     * key.  The translation key is the stable identity here: a player who
     * changed the binding to another key must not have that choice rewritten.
     *
     * <p>The caller owns persistence (normally {@code GameOptions#setKeyCode})
     * and must actually apply the replacement to the supplied binding.  The
     * registry rebuilds Minecraft's physical-key index after a successful
     * migration so the old key cannot continue to dispatch to the stale
     * binding during this client session.</p>
     *
     * @return {@code true} when the exact legacy binding was migrated;
     *         {@code false} when the current binding is already something else
     */
    public synchronized boolean migrateLegacyBoundKey(
        String translationKey,
        InputUtil.Key legacyKey,
        InputUtil.Key replacementKey,
        BiConsumer<KeyBinding, InputUtil.Key> rebinder
    ) {
        requireNonBlank(translationKey, "translation key");
        Objects.requireNonNull(legacyKey, "legacy key must not be null");
        Objects.requireNonNull(replacementKey, "replacement key must not be null");
        Objects.requireNonNull(rebinder, "rebinder must not be null");

        Registration registration = registrations.stream()
            .filter(candidate -> candidate.spec().translationKey().equals(translationKey))
            .findFirst()
            .orElseThrow(() -> new IllegalArgumentException(
                "cannot migrate unregistered translation key: " + translationKey
            ));
        KeyBinding binding = registration.binding();
        if (!isBoundTo(binding, legacyKey)) {
            return false;
        }

        rebinder.accept(binding, replacementKey);
        if (!isBoundTo(binding, replacementKey)) {
            throw new IllegalStateException(
                "rebinder did not apply replacement key for: " + translationKey
            );
        }
        KeyBinding.updateKeysByCode();
        return true;
    }

    private static boolean isBoundTo(KeyBinding binding, InputUtil.Key key) {
        if (key.equals(InputUtil.UNKNOWN_KEY)) {
            return binding.isUnbound();
        }
        return switch (key.getCategory()) {
            case KEYSYM -> binding.matchesKey(key.getCode(), 0);
            case SCANCODE -> binding.matchesKey(InputUtil.UNKNOWN_KEY.getCode(), key.getCode());
            case MOUSE -> binding.matchesMouse(key.getCode());
        };
    }

    private void rejectReservedConflicts(BindingOwner owner, PhysicalDefault key) {
        for (ReservedDefault reserved : reservedDefaults) {
            if (reserved.key().equals(key) && !isExempt(owner, reserved.owner(), key)) {
                throw physicalConflict(owner, reserved.owner(), key);
            }
        }
    }

    private void rejectRegistrationConflicts(BindingOwner owner, PhysicalDefault key) {
        for (Registration existing : registrations) {
            PhysicalDefault existingKey = new PhysicalDefault(
                existing.spec().type(), existing.spec().defaultCode()
            );
            if (existingKey.equals(key) && !isExempt(owner, existing.owner(), key)) {
                throw physicalConflict(owner, existing.owner(), key);
            }
        }
    }

    private boolean isExempt(BindingOwner first, BindingOwner second, PhysicalDefault key) {
        return exemptions.stream().anyMatch(exemption ->
            exemption.key().equals(key)
                && sameOwnerPair(exemption.firstOwner(), exemption.secondOwner(), first, second)
        );
    }

    private static boolean sameOwnerPair(
        BindingOwner firstLeft,
        BindingOwner secondLeft,
        BindingOwner firstRight,
        BindingOwner secondRight
    ) {
        return (firstLeft.equals(firstRight) && secondLeft.equals(secondRight))
            || (firstLeft.equals(secondRight) && secondLeft.equals(firstRight));
    }

    private static boolean participatesInPhysicalConflict(PhysicalDefault key) {
        return key.code() != InputUtil.UNKNOWN_KEY.getCode();
    }

    private static IllegalArgumentException physicalConflict(
        BindingOwner owner,
        BindingOwner conflictingOwner,
        PhysicalDefault key
    ) {
        return new IllegalArgumentException(
            "physical default conflict for " + owner.id() + " and " + conflictingOwner.id()
                + " at " + key.type() + "+" + key.code()
        );
    }

    private static String requireNonBlank(String value, String field) {
        Objects.requireNonNull(value, field + " must not be null");
        if (value.isBlank()) {
            throw new IllegalArgumentException(field + " must not be blank");
        }
        return value;
    }

    public record BindingOwner(String id) {
        public BindingOwner {
            requireNonBlank(id, "owner id");
        }
    }

    public record BindingSpec(
        BindingOwner owner,
        String translationKey,
        InputUtil.Type type,
        int defaultCode,
        String category
    ) {
        public BindingSpec {
            Objects.requireNonNull(owner, "owner must not be null");
            requireNonBlank(translationKey, "translation key");
            Objects.requireNonNull(type, "input type must not be null");
            requireNonBlank(category, "category");
        }
    }

    public record Registration(BindingOwner owner, BindingSpec spec, KeyBinding binding) {
        public Registration {
            Objects.requireNonNull(owner, "owner must not be null");
            Objects.requireNonNull(spec, "spec must not be null");
            Objects.requireNonNull(binding, "binding must not be null");
            if (!owner.equals(spec.owner())) {
                throw new IllegalArgumentException("registration owner must match spec owner");
            }
        }
    }

    public record PhysicalDefault(InputUtil.Type type, int code) {
        public PhysicalDefault {
            Objects.requireNonNull(type, "input type must not be null");
        }
    }

    public record ReservedDefault(BindingOwner owner, PhysicalDefault key) {
        public ReservedDefault {
            Objects.requireNonNull(owner, "reserved owner must not be null");
            Objects.requireNonNull(key, "reserved key must not be null");
        }
    }

    public record ConflictExemption(
        BindingOwner firstOwner,
        BindingOwner secondOwner,
        PhysicalDefault key,
        String reason
    ) {
        public ConflictExemption {
            Objects.requireNonNull(firstOwner, "first owner must not be null");
            Objects.requireNonNull(secondOwner, "second owner must not be null");
            Objects.requireNonNull(key, "exemption key must not be null");
            requireNonBlank(reason, "exemption reason");
        }
    }
}
