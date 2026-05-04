package com.bong.client.cultivation;

import com.bong.client.inventory.model.MeridianBody;
import org.junit.jupiter.api.Test;

import java.util.EnumMap;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

class QiColorVectorHudTest {
    @Test
    void missingColorsReportsAbsentPracticeWeights() {
        EnumMap<ColorKind, Double> weights = new EnumMap<>(ColorKind.class);
        weights.put(ColorKind.Heavy, 60.0);
        weights.put(ColorKind.Solid, 40.0);

        var missing = QiColorVectorHud.missingColors(weights);

        assertFalse(missing.contains(ColorKind.Heavy));
        assertFalse(missing.contains(ColorKind.Solid));
        assertEquals(ColorKind.Sharp, missing.get(0));
    }

    @Test
    void hunyuanDistanceTextShowsCompleteVector() {
        EnumMap<ColorKind, Double> weights = new EnumMap<>(ColorKind.class);
        for (ColorKind color : ColorKind.values()) {
            weights.put(color, 10.0);
        }
        MeridianBody body = MeridianBody.builder()
            .qiColorPracticeWeights(weights)
            .build();

        assertEquals("色种已齐", QiColorVectorHud.hunyuanDistanceText(body));
    }
}
