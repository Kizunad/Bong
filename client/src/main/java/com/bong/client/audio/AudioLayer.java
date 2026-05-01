package com.bong.client.audio;

import net.minecraft.util.Identifier;

public record AudioLayer(Identifier sound, float volume, float pitch, int delayTicks) {
}
