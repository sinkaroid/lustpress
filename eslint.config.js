const tsParser = require("@typescript-eslint/parser");
const tsPlugin = require("@typescript-eslint/eslint-plugin");

module.exports = [
  {
    ignores: ["node_modules/**", "build/**"],
  },
  {
    files: ["**/*.ts"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 12,
        sourceType: "module",
      },
      globals: {
        module: "readonly",
        require: "readonly",
        __dirname: "readonly",
        __filename: "readonly",
        process: "readonly",
        Buffer: "readonly",
        console: "readonly",
      },
    },
    plugins: {
      "@typescript-eslint": tsPlugin,
    },
    rules: {
      "no-unused-vars": "error",
      "no-undef": "error",
      "linebreak-style": 0,
      "quotes": ["error", "double"],
      "semi": ["error", "always"],
      "no-empty": "error",
      "no-func-assign": "error",
      "no-case-declarations": "off",
      "no-unreachable": "error",
      "no-eval": "error",
      "no-global-assign": "error",
      "@typescript-eslint/no-explicit-any": "off",
      "indent": ["error", 2],
    },
  },
];