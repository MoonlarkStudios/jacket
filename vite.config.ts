import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 1420,
    strictPort: true,
    // Native builds stage thousands of runtime files; they must not reload
    // the review workspace or consume the frontend's file-watcher budget.
    watch: { ignored: ['**/src-tauri/**', '**/target/**'] },
  },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    target: 'es2022',
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name: 'vendor',
              test: /[\\/]node_modules[\\/](?:react(?:-dom)?|@mantine[\\/](?:core|hooks))[\\/]/,
            },
          ],
        },
      },
    },
  },
});
