import js from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import globals from 'globals'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  { ignores: ['dist', 'src/bindings'] },
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: { globals: globals.browser },
    extends: [js.configs.recommended, ...tseslint.configs.strict, reactHooks.configs.flat.recommended],
    rules: {
      '@typescript-eslint/no-explicit-any': 'error',
    },
  },
  {
    files: ['*.js'],
    languageOptions: { globals: globals.node },
  },
)
