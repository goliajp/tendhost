import { readdirSync } from 'node:fs'
import reactHooks from 'eslint-plugin-react-hooks'
import tseslint from 'typescript-eslint'

// The layering of src/:
//   kit      shared pieces; imports nothing of ours but kit
//   modules  one directory per module; a module imports kit and itself only
//   app      the shell; the only place that knows every module
const modules = readdirSync(new URL('./src/modules', import.meta.url), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)

const anyPathTo = (dir) => [`@/${dir}`, `@/${dir}/**`, `**/${dir}/**`, `**/${dir}`]

const boundary = (files, forbidden, message) => ({
  files,
  rules: {
    'no-restricted-imports': ['error', { patterns: [{ group: forbidden, message }] }],
  },
})

export default tseslint.config(
  { ignores: ['dist', 'node_modules'] },
  ...tseslint.configs.recommended,
  {
    plugins: { 'react-hooks': reactHooks },
    rules: { 'react-hooks/rules-of-hooks': 'error', 'react-hooks/exhaustive-deps': 'error' },
  },
  boundary(
    ['src/kit/**'],
    [...anyPathTo('modules'), ...anyPathTo('app')],
    'kit is shared by every module and must not depend on a module or on the app shell',
  ),
  ...modules.map((name) =>
    boundary(
      [`src/modules/${name}/**`],
      [
        ...anyPathTo('app'),
        ...modules
          .filter((other) => other !== name)
          .flatMap((other) => [`@/modules/${other}`, `@/modules/${other}/**`, `**/${other}/**`, `**/${other}`]),
      ],
      'a module imports kit and itself only; what it needs from another module comes through a slot or the app shell',
    ),
  ),
)
