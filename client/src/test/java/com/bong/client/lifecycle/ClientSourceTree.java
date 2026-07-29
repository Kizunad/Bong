package com.bong.client.lifecycle;

import java.nio.file.Files;
import java.nio.file.Path;

final class ClientSourceTree {
    private ClientSourceTree() {
    }

    static Path clientRoot() {
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
        throw new AssertionError(
            "无法从当前目录向上定位 client/src/main/java/com/bong/client，实际 user.dir="
                + Path.of("").toAbsolutePath().normalize()
        );
    }
}
