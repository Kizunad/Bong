package com.bong.client.util;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class RealmLabelTest {
    @Test
    void displayNameMapsCanonicalRealmWireValues() {
        assertEquals("醒灵", RealmLabel.displayName("Awaken"));
        assertEquals("引气", RealmLabel.displayName("Induce"));
        assertEquals("凝脉", RealmLabel.displayName("Condense"));
        assertEquals("固元", RealmLabel.displayName("Solidify"));
        assertEquals("通灵", RealmLabel.displayName("Spirit"));
        assertEquals("化虚", RealmLabel.displayName("Void"));
    }

    @Test
    void displayNameKeepsUnknownValuesVisible() {
        assertEquals("凡体", RealmLabel.displayName("  "));
        assertEquals("HalfStepVoid", RealmLabel.displayName(" HalfStepVoid "));
    }
}
