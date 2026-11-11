# Threadkeep interface

Svelte 5, Vite, TypeScript. Owner: Claude 二.

```
npm install
npm run dev:demo     # invented data, no backend needed
npm run check        # type check
npm run build        # static build in dist/
```

Against the real backend: start `threadkeep app --no-open --port 5301`, then run
`THREADKEEP_URL=http://127.0.0.1:5301 npm run dev` and open the dev page with the `#t=...`
part of the address the backend printed.

`src/backend.ts` is the only file that knows the transport; it holds the route table from
`src/api.mjs`. `src/lib/types.ts` mirrors what `src/ops.mjs` returns. `src/demo.ts` is invented data.
