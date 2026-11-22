// Run with playwright-cli on a demo built with CTE (see docs/WASM.md).
async page => {
  await page.setViewportSize({ width: 1280, height: 800 });
  const webgl2 = await page.evaluate(() => new URL(location.href).searchParams.has('webgl2'));
  if (webgl2) {
    await page.addInitScript(() => Object.defineProperty(navigator, 'gpu', { value: undefined }));
  }
  await page.reload();
  const provider = await page.evaluateHandle(() => import('imgui-sys-v1'));
  await page.waitForFunction(p => p.igGetCurrentContext() && p.igGetFrameCount() > 2, provider);
  if (webgl2 && !await page.locator('#wasm-canvas').evaluate(canvas => !!canvas.getContext('webgl2'))) {
    throw new Error('Expected the WebGL2 canvas backend');
  }
  const waitForRender = async () => {
    const frame = await provider.evaluate(p => p.igGetFrameCount());
    await page.waitForFunction(({ p, frame }) => p.igGetFrameCount() > frame + 2,
      { p: provider, frame });
  };

  await provider.evaluate(async p => {
    const { default: init } = await import('./dear-imgui-web-demo.js');
    const rust = await init({ memory: p.memory });
    if (rust.memory !== p.memory) throw new Error('Rust and C++ have different memories');

    try {
      p.memory.grow(1);
      throw new Error('The Rust System allocator could grow into provider memory');
    } catch (error) {
      if (!(error instanceof RangeError)) throw error;
    }
    const size = 72 * 1024 * 1024;
    const native = p.igMemAlloc(32);
    const small = rust.__wbindgen_malloc(33, 4096);
    new Uint8Array(p.memory.buffer, native, 32).fill(0xa5);
    new Uint8Array(p.memory.buffer, small, 33).fill(0x5a);
    const large = rust.__wbindgen_realloc(small, 33, size, 4096);
    try {
      const buffer = p.memory.buffer;
      if (Math.min(native, small, large) < 64 * 1024 * 1024 ||
          small % 4096 || large % 4096 || (native < large + size && large < native + 32) ||
          !new Uint8Array(buffer, native, 32).every(b => b === 0xa5) ||
          !new Uint8Array(buffer, large, 33).every(b => b === 0x5a)) {
        throw new Error('Provider heap ranges, alignment or allocation contents changed');
      }
      // The all-extensions demo initially places other ImGui windows over the editor.
      new Uint8Array(buffer, native, 32).set(new TextEncoder().encode('ImGuiColorTextEdit (Web)\0'));
      if (!p.igFindWindowByName(native)) throw new Error('Build the web demo with cte');
      p.igSetWindowFocus_Str(native);
    } finally {
      rust.__wbindgen_free(large, size, 4096);
      p.igMemFree(native);
    }
  });

  // Only the transaction count changes in this row. Read its rendered pixels
  // to exercise the native editor, Rust callback and renderer without test hooks.
  const counter = { clip: { x: 68, y: 90, width: 530, height: 22 } };
  await waitForRender();
  await page.mouse.click(150, 180);
  const before = await page.screenshot(counter);
  await page.keyboard.press('x');
  await waitForRender();
  if (before.equals(await page.screenshot(counter))) {
    throw new Error('Editing the CTE document did not update the rendered transaction count');
  }
  return true;
}
