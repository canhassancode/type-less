import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    projects: ['modules/levelling', 'modules/activation', 'modules/theme-loader', 'modules/app'],
    passWithNoTests: true,
  },
});
