import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// `npm run dev` forwards /api to the local Threadkeep server (`threadkeep app`).
// Set THREADKEEP_URL to the address that command prints, without the #t=... part.
// changeOrigin is needed: the server refuses requests whose Host is not its own.
const target = process.env.THREADKEEP_URL;

export default defineConfig({
  base: './',
  plugins: [svelte()],
  server: {
    port: Number(process.env.PORT) || 5199,
    proxy: target ? { '/api': { target, changeOrigin: true } } : undefined,
  },
});
