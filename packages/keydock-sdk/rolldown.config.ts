import { defineConfig } from "rolldown";
import { dts } from "rolldown-plugin-dts";

const external = ["ky"];

export default defineConfig([
  {
    input: "src/index.ts",
    external,
    output: {
      dir: "dist",
      format: "esm",
      entryFileNames: "index.js",
      sourcemap: false,
      cleanDir: true,
    },
  },
  {
    input: {
      index: "src/index.ts",
    },
    external,
    plugins: [
      dts({
        emitDtsOnly: true,
        resolver: "tsc",
        sourcemap: false,
      }),
    ],
    output: {
      dir: "dist",
      format: "esm",
      sourcemap: false,
    },
  },
]);
