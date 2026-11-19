// The core with its split modules (K4, MS4 step 20, D230) in the harness's
// workers, as tools/bench/run.mjs' instantiateCore gives it in Node: each
// placeholder the core imports (`placeholder.<module>`) throws, as the
// studio's driver's do until it loads the module, and every split module
// /scheme/manifest.json names is fetched once from /scheme/ and joined to
// each instance, so the instance is the whole core. A worker loads it with
// importScripts. `splitLoads` (MS4 step 21): each module's bytes, its fetch
// and compile once, and each join to an instance, in ms.
self.splitLoads = {}
self.instantiateCore = (() => {
  let modules
  const split = () => (modules ??= fetch('scheme/manifest.json')
    .then((response) => response.json())
    .then((manifest) => Promise.all(Object.entries(manifest.modules).filter(([, m]) => m.kind === 'split')
      .map(async ([name, m]) => {
        const t = performance.now()
        const bytes = await (await fetch(`scheme/${m.file}`)).arrayBuffer()
        const u = performance.now()
        const module = await WebAssembly.compile(bytes)
        splitLoads[name] = { bytes: bytes.byteLength, fetchMs: u - t, compileMs: performance.now() - u, joinMs: [] }
        return [name, module]
      }))))
  return async (mod) => {
    const imports = { env: { lily_now: () => performance.now() } }
    for (const i of WebAssembly.Module.imports(mod)) {
      if (!i.module.startsWith('placeholder.')) continue
      imports[i.module] ??= {}
      imports[i.module][i.name] = () => { throw new Error(`the core called ${i.module} ${i.name}, which is not loaded`) }
    }
    const instance = await WebAssembly.instantiate(mod, imports)
    for (const [name, module] of await split()) {
      const t = performance.now()
      await WebAssembly.instantiate(module, { primary: instance.exports })
      splitLoads[name].joinMs.push(performance.now() - t)
    }
    return instance
  }
})()
