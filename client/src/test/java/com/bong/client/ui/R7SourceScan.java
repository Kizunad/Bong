package com.bong.client.ui;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

final class R7SourceScan {
    private R7SourceScan() {
    }

    static Path productionRoot() {
        Path workingDirectory = Path.of("").toAbsolutePath().normalize();
        Path clientRoot = Files.isDirectory(workingDirectory.resolve("src"))
            ? workingDirectory
            : workingDirectory.resolve("client");
        return clientRoot.resolve("src/main/java/com/bong/client");
    }

    static Path repositoryRoot() {
        return productionRoot()
            .getParent()
            .getParent()
            .getParent()
            .getParent()
            .getParent()
            .getParent()
            .getParent();
    }

    static String read(Path path) {
        try {
            return Files.readString(path);
        } catch (IOException exception) {
            throw new AssertionError("无法读取 R7 contract source：" + path.toAbsolutePath(), exception);
        }
    }

    static List<TokenOccurrence> tokenOccurrences(Path root, String token) throws IOException {
        List<TokenOccurrence> occurrences = new ArrayList<>();
        try (var files = Files.walk(root)) {
            for (Path path : files.filter(Files::isRegularFile)
                .filter(candidate -> candidate.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList()) {
                occurrences.addAll(tokenOccurrences(root, path, token));
            }
        }
        return occurrences;
    }

    static String codeOnly(String source) {
        StringBuilder result = new StringBuilder(source.length());
        LexState state = LexState.CODE;
        for (int index = 0; index < source.length(); index++) {
            char current = source.charAt(index);
            char next = index + 1 < source.length() ? source.charAt(index + 1) : '\0';
            switch (state) {
                case CODE -> {
                    if (current == '/' && next == '/') {
                        result.append("  ");
                        index++;
                        state = LexState.LINE_COMMENT;
                    } else if (current == '/' && next == '*') {
                        result.append("  ");
                        index++;
                        state = LexState.BLOCK_COMMENT;
                    } else if (current == '"') {
                        result.append(' ');
                        state = LexState.STRING;
                    } else if (current == '\'') {
                        result.append(' ');
                        state = LexState.CHARACTER;
                    } else {
                        result.append(current);
                    }
                }
                case LINE_COMMENT -> {
                    result.append(current == '\n' ? '\n' : ' ');
                    if (current == '\n') {
                        state = LexState.CODE;
                    }
                }
                case BLOCK_COMMENT -> {
                    if (current == '*' && next == '/') {
                        result.append("  ");
                        index++;
                        state = LexState.CODE;
                    } else {
                        result.append(current == '\n' ? '\n' : ' ');
                    }
                }
                case STRING, CHARACTER -> {
                    result.append(current == '\n' ? '\n' : ' ');
                    if (current == '\\' && next != '\0') {
                        result.append(next == '\n' ? '\n' : ' ');
                        index++;
                    } else if ((state == LexState.STRING && current == '"')
                        || (state == LexState.CHARACTER && current == '\'')) {
                        state = LexState.CODE;
                    }
                }
            }
        }
        return result.toString();
    }

    private static List<TokenOccurrence> tokenOccurrences(Path root, Path path, String token) {
        String source = read(path);
        String[] lines = source.split("\\R", -1);
        List<TokenOccurrence> occurrences = new ArrayList<>();
        LexState state = LexState.CODE;
        int line = 1;
        int ordinal = 0;

        for (int index = 0; index < source.length();) {
            if (source.startsWith(token, index)) {
                ordinal++;
                occurrences.add(new TokenOccurrence(
                    root.relativize(path).toString().replace('\\', '/'),
                    ordinal,
                    line,
                    state == LexState.CODE,
                    lines[line - 1].strip()
                ));
                index += token.length();
                continue;
            }

            char current = source.charAt(index);
            char next = index + 1 < source.length() ? source.charAt(index + 1) : '\0';
            switch (state) {
                case CODE -> {
                    if (current == '/' && next == '/') {
                        state = LexState.LINE_COMMENT;
                        index += 2;
                        continue;
                    }
                    if (current == '/' && next == '*') {
                        state = LexState.BLOCK_COMMENT;
                        index += 2;
                        continue;
                    }
                    if (current == '"') {
                        state = LexState.STRING;
                    } else if (current == '\'') {
                        state = LexState.CHARACTER;
                    }
                }
                case LINE_COMMENT -> {
                    if (current == '\n') {
                        state = LexState.CODE;
                    }
                }
                case BLOCK_COMMENT -> {
                    if (current == '*' && next == '/') {
                        state = LexState.CODE;
                        index += 2;
                        continue;
                    }
                }
                case STRING, CHARACTER -> {
                    if (current == '\\' && next != '\0') {
                        if (next == '\n') {
                            line++;
                        }
                        index += 2;
                        continue;
                    }
                    if ((state == LexState.STRING && current == '"')
                        || (state == LexState.CHARACTER && current == '\'')) {
                        state = LexState.CODE;
                    }
                }
            }
            if (current == '\n') {
                line++;
            }
            index++;
        }
        return occurrences;
    }

    record TokenOccurrence(String path, int ordinal, int line, boolean code, String sourceLine) {
        String stableKey() {
            return path + "#" + ordinal;
        }
    }

    private enum LexState {
        CODE,
        LINE_COMMENT,
        BLOCK_COMMENT,
        STRING,
        CHARACTER
    }
}
