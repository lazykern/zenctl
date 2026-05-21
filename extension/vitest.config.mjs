import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    setupFiles: ["./tests/setup.js"],
    include: ["tests/**/*.test.js"],
    globals: true,
    // background.js uses a CJS-like export at the bottom for Node.js testing
    // (Firefox ignores it because MV2 bg scripts don't have module.exports)
    server: {
      deps: {
        inline: [/background\.js$/],
      },
    },
  },
});
