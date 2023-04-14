import typescript from "@rollup/plugin-typescript";
import resolve from "@rollup/plugin-node-resolve";
import postcss from "rollup-plugin-postcss";
import html from "@open-wc/rollup-plugin-html";
import { terser } from "rollup-plugin-terser";
import copy from "rollup-plugin-copy";

const prod = process.env.NODE_ENV === "production";

export default {
  input: "./src/index.html",
  output: {
    dir: "dist",
    format: "iife",
  },
  plugins: [
    html(),
    typescript(),
    resolve(),
    postcss({
      extract: true,
      config: "postcss.config.js",
    }),
    copy({
      flatten: false,
      targets: [{ src: "images/**/*", dest: "dist/images/" }],
    }),
    ...(prod ? [terser()] : []),
  ],
};
