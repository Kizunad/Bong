package com.bong.client.menu;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

class MainMenuConfigTest {
    @TempDir Path directory;

    @Test
    void loadingConfigPreservesOperatorEditsEvenWhenMalformed() throws Exception {
        Path path = directory.resolve("config/bong-client.json");
        MainMenuConfig.load(path);
        String edited = "{\"serverAddress\":\"example.org:25566\",\"motion\":false,\"ambience\":false}";
        Files.writeString(path, edited);
        assertEquals(new MainMenuConfig("example.org:25566", false, false), MainMenuConfig.load(path));
        assertEquals(edited, Files.readString(path), "加载配置不能覆盖部署者的地址和视觉选项");
        String broken = "{\"serverAddress\":42,\"motion\":\"false\"}";
        Files.writeString(path, broken);
        assertThrows(IllegalArgumentException.class, () -> MainMenuConfig.load(path));
        assertEquals(broken, Files.readString(path), "配置无效时也不能擅自重置用户文件");
    }
}
