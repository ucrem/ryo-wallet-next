import js from "@eslint/js"
import tseslint from "typescript-eslint"
import reactHooks from "eslint-plugin-react-hooks"

export default [
  {
    ignores: [
      "dist/**",
      "target/**",
      "src-tauri/target/**",
      "src/components/ui/**",
    ],
  },

  js.configs.recommended,
  ...tseslint.configs.recommended,
  reactHooks.configs.flat.recommended,

  {
    files: ["scripts/**/*.mjs"],
    languageOptions: {
      globals: {
        process: "readonly",
        fetch: "readonly",
      },
    },
  },
]