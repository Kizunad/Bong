package com.bong.client.combat;

/**
 * Derived-attribute state flags surfaced in the combat HUD (§3.3).
 * Immutable; construct via {@link #of}.
 */
public final class DerivedAttrFlags {
    private static final DerivedAttrFlags NONE = new DerivedAttrFlags(false, false, false);

    private final boolean flying;
    private final boolean phasing;
    private final boolean tribulationLocked;

    private DerivedAttrFlags(boolean flying, boolean phasing, boolean tribulationLocked) {
        this.flying = flying;
        this.phasing = phasing;
        this.tribulationLocked = tribulationLocked;
    }

    public static DerivedAttrFlags none() {
        return NONE;
    }

    public static DerivedAttrFlags of(boolean flying, boolean phasing, boolean tribulationLocked) {
        if (!flying && !phasing && !tribulationLocked) {
            return NONE;
        }
        return new DerivedAttrFlags(flying, phasing, tribulationLocked);
    }

    public boolean flying() {
        return flying;
    }

    public boolean phasing() {
        return phasing;
    }

    public boolean tribulationLocked() {
        return tribulationLocked;
    }

    public boolean isEmpty() {
        return !flying && !phasing && !tribulationLocked;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof DerivedAttrFlags other)) return false;
        return flying == other.flying
            && phasing == other.phasing
            && tribulationLocked == other.tribulationLocked;
    }

    @Override
    public int hashCode() {
        int r = 0;
        if (flying) r |= 1;
        if (phasing) r |= 2;
        if (tribulationLocked) r |= 4;
        return r;
    }
}
