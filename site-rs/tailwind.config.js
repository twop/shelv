const { colors } = require("./src/theme");

/** @type {import('tailwindcss').Config} */
module.exports = {
  mode: "all",
  content: ["./src/**/*.{rs,html,css}", "./dist/**/*.html"],
  theme: {
    colors,
    extend: {
      borderWidth: {
        1: "1px",
      },
    },
  },
  plugins: [],
};
