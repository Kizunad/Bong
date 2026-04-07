package com.bong.client.network;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

final class PayloadFixtureLoader {
    private PayloadFixtureLoader() {
    }

    static String readText(String fileName) throws IOException {
        String resourcePath = "bong/payloads/" + fileName;
        try (InputStream inputStream = PayloadFixtureLoader.class.getClassLoader().getResourceAsStream(resourcePath)) {
            if (inputStream == null) {
                throw new IOException("Missing test fixture: " + resourcePath);
            }
            return new String(inputStream.readAllBytes(), StandardCharsets.UTF_8);
        }
    }

    static byte[] readHexBytes(String fileName) throws IOException {
        String hex = readText(fileName).replaceAll("\\s+", "");
        if (hex.length() % 2 != 0) {
            throw new IOException("Hex fixture length must be even: " + fileName);
        }

        byte[] bytes = new byte[hex.length() / 2];
        for (int index = 0; index < hex.length(); index += 2) {
            int high = Character.digit(hex.charAt(index), 16);
            int low = Character.digit(hex.charAt(index + 1), 16);
            if (high < 0 || low < 0) {
                throw new IOException("Invalid hex fixture content: " + fileName);
            }
            bytes[index / 2] = (byte) ((high << 4) + low);
        }
        return bytes;
    }
}
