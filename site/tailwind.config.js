const { colors } = require("./src/theme");

module.exports = {
  purge: {
    content: [
      "./src/**/*.html",
      "./src/**/*.js",
      "./src/**/*.jsx",
      "./src/**/*.ts",
      "./src/**/*.tsx",
    ],
    mode: "all",
  },
  theme: {
    colors,
    extend: {
      borderWidth: {
        1: "1px",
      },
    },
  },
  variants: {},
  plugins: [],
};
