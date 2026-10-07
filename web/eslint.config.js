// @ts-check
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default tseslint.config(
  { ignores: ["dist/**", "dist-admin/**", "dist-smoke/**", "public/**", "node_modules/**"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // rules/web.md: no XSS-prone APIs, no `any` without justification
      "@typescript-eslint/no-explicit-any": "error",
      "no-eval": "error",
      "no-implied-eval": "error",
      "no-new-func": "error",
      "no-restricted-properties": [
        "error",
        { object: "localStorage", property: "setItem", message: "No tokens/sessions in web storage (ADR-007)." },
        { object: "sessionStorage", property: "setItem", message: "No tokens/sessions in web storage (ADR-007)." },
      ],
      "no-restricted-syntax": [
        "error",
        { selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']", message: "Forbidden (rules/web.md)." },
      ],
    },
  },
);
