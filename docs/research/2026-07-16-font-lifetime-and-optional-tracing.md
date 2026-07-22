# Font lifetime, dependency, and optional tracing research

Date: 2026-07-16

Status: implemented decision record. Baseline sections preserve the verified pre-change behavior;
the final decisions below describe the selected Rust API and feature contracts.

## Scope and source baseline

This research covers:

- the deferred font-baked and custom-rectangle surface recorded by the safe API audit, together with
  font atlas, font, texture, and font-loader ownership in the pinned Dear ImGui/cimgui sources;
- the current Rust ownership model around those objects;
- [PR #42](https://github.com/Latias94/dear-imgui-rs/pull/42), including its actual diff,
  discussion state, and CI state;
- the direct dependency and feature graph of `dear-imgui-rs`, including propagation through
  backends and extensions; and
- which features participate in native prebuilt identity and verification.

The repository baseline is
[`3a748122d9c557b6229f258d2e2b9bacc4178e4a`](https://github.com/Latias94/dear-imgui-rs/tree/3a748122d9c557b6229f258d2e2b9bacc4178e4a).
It pins cimgui
[`1261b231939fc210032f30c4ee8a8f0440372237`](https://github.com/cimgui/cimgui/tree/1261b231939fc210032f30c4ee8a8f0440372237)
and nested Dear ImGui
[`b61e56346a92cfcaf1f43a545ca37b0b32239654`](https://github.com/ocornut/imgui/tree/b61e56346a92cfcaf1f43a545ca37b0b32239654).
Those revisions are also package metadata in
[`dear-imgui-sys/Cargo.toml`](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/Cargo.toml#L30-L37).

There was no existing `docs/research/` convention in this repository, so this record establishes
that directory.

## Implemented outcome

- `FontAtlas` is now a transparent context-borrowed view returned only as `&FontAtlas`;
  `SharedFontAtlas` is the only standalone Rust owner. Mutators use Dear ImGui's atlas lock as an
  interior-mutation protocol instead of claiming false Rust uniqueness. Shared contexts update
  externally managed atlases once per frame, require consistent renderer texture capability while
  registered, reject incompatible renderer-mode transitions before FFI, unregister before context
  destruction, and cannot be suspended with an open frame. Legacy-to-managed transitions require a
  clear and repopulation; managed-to-legacy transitions require a full legacy `build()`.
- `BakedFont<'ui>` is available only through `Ui` for the current font or an atlas-validated
  `FontId`, size, and density. Glyph data is copied into an owned `Glyph` containing stable metrics;
  the safe API omits UVs because another lazy glyph load can repack the atlas in the same frame.
  Legacy locked atlases reject creation of arbitrary new baked sizes before entering FFI. Old
  `Font` text measurement methods were removed; callers measure the current scoped font through
  `Ui`.
- `FontAtlas::tex_data()` returns a read-only `FontAtlasTexture<'_>` lease. Rebuilds, clears,
  custom-rectangle writes, and frame advancement reject a live lease rather than invalidating its
  pixel slice. Safe borrowed raw-texture views were removed. `FontAtlas::raw()` remains a raw
  pointer for explicit unsafe FFI interop; it does not expose a Rust reference or bypass the
  lifetime of the borrowed `FontAtlas` view.
- `FontSource` is opaque. External TTF/OTF, compressed, Base85, and file constructors and the direct
  raw-data add methods are unsafe because the native parsers do not consistently enforce an input
  boundary. Memory data is copied into Dear ImGui-owned storage, file sources are read before batch
  mutation, and structured include ranges are retained for the native source lifetime.
- Custom rectangles use atlas- and generation-validated `CustomRectId` values, strict alpha/RGBA
  pixel buffers, copy-out snapshots, exact texture update regions, and a `Ui::image_custom_rect`
  submission helper. Repacking preserves IDs; removal or builder clear invalidates them.
- Unknown-count clipping has a separate `ListClipper::unknown_count()` protocol with fused
  `next_range()` and consuming `finish(final_items_count)`. The known-count constructor rejects the
  native `INT_MAX` sentinel. Both protocols enforce native LIFO and the exact frame, window `Begin`,
  and table instance. Out-of-order destruction is deferred; wrong-scope and forgotten-token cleanup
  disables cursor seeking while native code restores its own temporary stack. `Context::render()`
  rejects and recovers a frame containing a forgotten token.
- Core no longer owns a tracing subscriber, exported logging macros, or error-construction side
  effects. Its default feature set and production dependency graph contain no tracing crates. WGPU
  exposes tracing as an opt-in feature, while native examples install their own subscriber; the
  browser example continues to use its web-console logger.
- Workspace dependencies disable core defaults explicitly. Unused direct `cfg-if`, `memoffset`,
  Ash `log`, and core production-only math edges were removed or moved to dev dependencies.
- The high-level crate forwards native `prebuilt` and `build-from-source` strategy features.
  `prebuilt` is native-only and keeps download/extraction crates in the host build graph;
  `test-engine` is source-only and rejected on WASM, and source building wins if Cargo unifies it
  with `prebuilt`. Packaged-prebuilt verification rejects unknown feature profiles and consumes
  artifacts through the high-level `prebuilt` feature.

## Existing API-audit boundary

The top-level safe audit classifies only `ImGui::GetFontBaked` as `deferred-design`, because a safe
view has to account for atlas mutation, density, and context-owned lifetime
([policy](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/tools/api_surface_policy.json#L69-L77)).
The coverage document separately calls out the namespaced `ImFont::GetFontBaked`, `ImFontBaked`
queries, custom rectangles, and the unknown-count list-clipper seek operation
([coverage notes](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/docs/API_COVERAGE.md#L61-L76)).

This distinction matters: the audit automatically decides safe coverage only for top-level
`ImGui` functions. Namespaced declarations are snapshot-guarded against upstream drift but still
require manual safe-layer review
([coverage contract](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/docs/API_COVERAGE.md#L35-L59)).
The relevant cimgui functions are thin calls into C++ rather than independent ownership APIs
([top-level and font wrappers](https://github.com/cimgui/cimgui/blob/1261b231939fc210032f30c4ee8a8f0440372237/cimgui.cpp#L432-L436),
[atlas, baked, and font wrappers](https://github.com/cimgui/cimgui/blob/1261b231939fc210032f30c4ee8a8f0440372237/cimgui.cpp#L2894-L2985)).

## Baseline Rust atlas ownership risk

Before this implementation, the Rust layer used one `FontAtlas` type for two different roles:

- `FontAtlas::new()` creates and owns an `ImFontAtlas`, while `Drop` destroys it
  ([implementation](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/lifecycle.rs#L10-L25),
  [drop](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/lifecycle.rs#L69-L78)).
- `FontAtlas::from_raw()` creates the same type with `owned = false`
  ([implementation](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/lifecycle.rs#L35-L51)).
- The type stores a raw pointer, an `owned` boolean, and `PhantomData<*mut ImFontAtlas>`; it has no
  borrow lifetime
  ([definition](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core.rs#L19-L25)).
- `Context::font_atlas_mut()` and its `fonts()` alias return that non-owning `FontAtlas` by value,
  without tying the returned value to the `Context` borrow
  ([context methods](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/context/fonts.rs#L124-L164)).
- The read-only path already uses a distinct `FontAtlasRef<'atlas>` whose phantom lifetime is tied
  to its source borrow
  ([definition](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core.rs#L27-L35),
  [context method](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/context/fonts.rs#L84-L120)).

### Observed risks

1. Safe code can retain the value returned by `Context::font_atlas_mut()`, drop its owning context,
   and then call a safe `FontAtlas` method through a dangling raw pointer. The generation registry
   protects `FontId` validation, but `FontAtlas` itself carries neither the registry stamp nor a
   context lifetime
   ([font ID fields and validation](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/id.rs#L9-L76),
   [context teardown registration cleanup](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/context/core.rs#L237-L249)).
2. Multiple contexts may intentionally share one atlas, but each context can produce an independent
   non-owning `FontAtlas` value. The `Rc` in `SharedFontAtlas` keeps allocation ownership alive; it
   does not encode unique mutable borrowing among the independent views
   ([shared owner](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/shared.rs#L7-L46)).
3. Several safe atlas mutators rely on upstream assertions for the locked-atlas precondition,
   while only `compact_cache()` performs its own Rust-side `Locked` check
   ([Rust mutations](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/mutation.rs#L8-L42),
   [Rust cache operations](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/build.rs#L31-L74),
   [upstream lock checks](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L2693-L2756)).

These risks exist before adding baked-font or custom-rectangle wrappers. Any new borrowed view would
inherit the current atlas handle's lifetime unless that foundation is addressed or bypassed.

## Shared-atlas frame updates and ownership conflict

The pinned Dear ImGui revision changed the obligations of application-owned/shared font atlases in
ways the current Rust wrapper does not model:

- A context-created atlas records that context as `OwnerContext`, but an atlas passed to
  `CreateContext()` remains externally managed
  ([context construction](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.cpp#L4244-L4265)).
- `NewFrame()` updates an atlas automatically only when the current context is its owner. For an
  externally managed/shared atlas, the application must call `ImFontAtlasUpdateNewFrame()` first,
  with a monotonically increasing frame count and the same `RendererHasTextures` capability used by
  every context sharing that atlas
  ([new-frame check](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.cpp#L9577-L9597),
  [atlas update contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L2774-L2795)).
- Context initialization registers its atlas and increments `ImFontAtlas::RefCount`; unregistering
  removes the draw-list association and decrements it
  ([registration](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.cpp#L9675-L9695)).
- During context shutdown, Dear ImGui unregisters every registered atlas and deletes an atlas when
  its reference count reaches zero, regardless of the Rust-side `Rc`
  ([shutdown](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.cpp#L4557-L4578)).

The Rust `SharedFontAtlas` separately destroys the same native pointer when its last `Rc` is dropped
([Rust drop](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/shared.rs#L33-L45)),
while `Context::drop()` currently calls `igDestroyContext()` without first detaching the shared atlas
from that context
([context drop](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/context/core.rs#L235-L258)).
After a shared-atlas context has initialized, the native shutdown path can therefore delete the
atlas before the final Rust owner runs, leaving the Rust handle dangling and making its eventual
drop a second destruction. The existing shared-atlas test only creates a raw atlas view; it never
attaches the atlas to a context or opens a frame
([test](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/tests.rs#L239-L249)).

This makes shared-atlas coordination part of the ownership prerequisite, not an optional follow-up
to baked-font access. A corrected implementation must both drive the atlas-wide frame counter and
prevent native context shutdown from racing or duplicating the Rust owner's destruction.

## `ImFontBaked` ownership and invalidation

### Upstream ownership facts

- `ImFontBaked` is runtime cache data for one font size and rasterizer density. The public header
  explicitly states that its pointers are valid only for the current frame
  ([type declaration](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3963-L3995)).
- Each baked object points back to its `OwnerFont`; its per-source loader storage is a single
  core-allocated buffer
  ([fields](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3978-L3988)).
- The atlas builder owns all baked objects in `ImStableVector<ImFontBaked, 32>` and indexes them by
  a hash of font ID, rounded size, and density
  ([builder storage](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_internal.h#L4190-L4217),
  [lookup and creation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L5400-L5469)).
- `ImStableVector` prevents growth from invalidating pointers, but its own contract says `clear()`
  invalidates them
  ([container contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_internal.h#L738-L769)).
- At the start of a new frame, Dear ImGui clears every font's `LastBaked` cache and compacts
  discarded entries by copying live objects to lower addresses. The source comment explicitly
  relies on baked pointers never crossing frames
  ([new-frame maintenance](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L2774-L2822)).

### Operations that invalidate or semantically retire baked views

| Operation | Verified effect |
| --- | --- |
| New frame | Discarded entries may be compacted and moved; no pointer may cross the boundary ([source](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L2797-L2822)). |
| Explicit discard or cache pressure | `ImFontAtlasBakedDiscard` destroys loader state, removes the map entry, clears output, and marks the object for compaction; texture-space pressure discards bakes unused for two frames ([discard](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3946-L4002), [space pressure](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L4218-L4232)). |
| Merge another source | Adding a merge source discards all bakes for the destination font because the per-baked/per-source loader buffer size changes ([source](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3060-L3090)). |
| Change default loader | All font output is destroyed before the old loader is shut down, then initialized again under the new loader ([source](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3494-L3527)). |
| Clear font output, fonts, or builder | `ImFont::ClearOutputData` discards its bakes; `ClearFonts` destroys the builder and all fonts ([font clear](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L5198-L5215), [atlas clear](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L2705-L2737)). |
| Rebuild after texture-format change | `ImFontAtlasBuildMain` calls `ImFontAtlasBuildClear`, which destroys and recreates the builder and all cached output ([build path](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3468-L3487), [clear path](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L4268-L4279)). |

`ImFontFlags_LockBakedSizes` disables loading new sizes and garbage collecting existing ones, but the
public pointer contract remains current-frame-only
([flag documentation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3998-L4007)).
It therefore cannot, by itself, justify a longer safe Rust lifetime.

### Query-method mutation

`FindGlyph`, `FindGlyphNoFallback`, and `GetCharAdvance` may lazily load a glyph. Loading can pack a
new rectangle and trigger atlas texture movement; returned `ImFontGlyph*` values are pointers into
the baked object's `Glyphs` vector
([public baked methods](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3989-L3995),
[lazy advance path](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L5384-L5399)).
Another glyph insertion may reallocate that vector. This makes a copied glyph value materially
different from a borrowed glyph reference.

The current Rust `Glyph` already owns a copy of `ImFontGlyph`
([implementation](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/glyph.rs#L8-L50)).
The current `Ui` is documented as one-frame state and is borrowed from `Context::frame()`
([`Ui`](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/ui.rs#L34-L42),
[`Context::frame`](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/context/frame.rs#L121-L151)).
The implementation selects that boundary directly: `BakedFont<'ui>` borrows the current `Ui`, and
all glyph queries copy values out instead of returning native references.

## Custom rectangle identity, snapshots, and pixel upload

### Identity and snapshot rules

- `ImFontAtlasRectId` is an opaque integer; `-1` is invalid. A rectangle may move, and the header
  requires callers to retrieve current coordinates through `GetCustomRect`
  ([public contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3769-L3784)).
- Internally, the ID contains an index and generation. The stable index entry survives rectangle
  reordering, while removal increments the generation so stale IDs can be rejected
  ([ID representation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_internal.h#L4143-L4161),
  [discard behavior](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L4398-L4433)).
- `GetCustomRect` copies x/y/width/height and UVs into caller storage; the snapshot is calculated
  against the current texture and UV scale
  ([implementation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3431-L3446)).
- The public API warns that any ImGui or atlas function may create or resize a texture and invalidate
  rectangle position and UV data
  ([custom-rectangle contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3875-L3898)).
- Repacking preserves active IDs by reusing index entries, but moves rectangles and refreshes glyph
  UVs and other cached UV data
  ([repack implementation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L4116-L4185)).
- Removing a rectangle invalidates its ID but leaves old pixels until a resize or garbage collection.
  Destroying/recreating the atlas builder invalidates all custom-rectangle IDs
  ([remove and get](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3374-L3380),
  [builder-clear contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L4268-L4279)).

These facts support a persistent typed ID and non-persistent copy-out snapshots. They do not support
storing a borrowed `ImFontAtlasRect` or treating UVs as properties of the ID.

### Pixel-write and renderer-update rules

Since Dear ImGui 1.92, `AddCustomRect` packs immediately. If the renderer supports managed texture
updates, the call queues that region for upload before returning
([implementation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3351-L3371)).
The 1.92 changelog says callers may write pixels immediately and that rectangles may move during a
texture change, practically at any time
([upstream changelog](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/docs/CHANGELOG.txt#L1588-L1604)).

The queued update is a copied rectangle in `ImTextureData::Updates`, and queuing changes the texture
status to `WantUpdates` where appropriate
([queue implementation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L2990-L3019)).
Writing an already-existing custom rectangle later does not itself call this queue operation; the
public custom-rectangle API exposes no separate upload-notification function.

The current Rust texture surface exposes immutable pixel slices and a whole-texture `set_data()`
operation, but no bounded mutable pixel region
([pixel access](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/texture/data.rs#L202-L228),
[whole-texture update](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/texture/data.rs#L315-L365)).
`FontAtlas::tex_data_mut()` exposes the entire current texture through the already-unbound
`FontAtlas` handle
([implementation](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/texture.rs#L76-L86)).

The implementation combines allocation with the first validated pixel write and also supports
later full-rectangle replacement. Each write resolves the current placement, copies or converts the
complete region, and queues that exact region before returning; no mutable pixel-region borrow is
exposed across an operation that could move the texture.

## Font-loader ownership and rebuild behavior

`ImFontLoader` is declared in `imgui_internal.h`, not the stable public header. Upstream describes
it as likely to evolve while incremental atlas updates are developed
([interface declaration](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_internal.h#L4098-L4126)).

The callback/storage ownership observed in the pinned implementation is:

| Storage | Owner/lifecycle |
| --- | --- |
| `const ImFontLoader*` callback table | Stored by pointer in the atlas or source config; Dear ImGui does not copy or destroy the table ([atlas and source fields](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3718-L3729), [atlas fields](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3937-L3944)). |
| `ImFontAtlas::FontLoaderData` | Loader callbacks create and destroy this atlas-wide state through `LoaderInit`/`LoaderShutdown`; the FreeType loader is a concrete example ([FreeType state](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/misc/freetype/imgui_freetype.cpp#L148-L178), [init/shutdown](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/misc/freetype/imgui_freetype.cpp#L348-L400)). |
| `ImFontConfig::FontLoaderData` | Per-source state is created/destroyed by `FontSrcInit`/`FontSrcDestroy` ([FreeType implementation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/misc/freetype/imgui_freetype.cpp#L402-L425)). |
| `ImFontBaked::FontLoaderDatas` | One contiguous per-baked/per-source buffer is allocated by core using each loader's `FontBakedSrcLoaderDataSize`, then passed to baked callbacks ([interface contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_internal.h#L4110-L4121), [discard cleanup](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3946-L3973)). |

The built-in stb loader is a function-static callback table
([implementation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L4841-L4852)).
The Rust wrapper therefore returns it as `&'static FontLoader`. For external native loaders,
`FontLoader::from_raw` is unsafe and requires the caller to uphold the callback table, name,
userdata, ABI, and no-unwind contract; safe atlas/config setters then require a `'static` loader
([Rust loader wrapper](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/loader.rs#L3-L42),
[atlas setter](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/loader_settings.rs#L6-L16),
[per-source setter](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/config.rs#L197-L205)).

Upstream supports changing the default loader at runtime. It first destroys all font output under
the old source/default loader selection, shuts down the old atlas loader, stores the new pointer,
initializes it, and rebuilds source output
([implementation](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3494-L3527)).
The Rust setter's documentation currently says it must be called before adding fonts, which is more
restrictive than the upstream behavior
([Rust documentation](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/core/loader_settings.rs#L6-L13)).

`FontLoaderFlags` is exposed on the generic atlas/config API but its Rust constants and docs mirror
FreeType-specific flags. Upstream defines the storage as loader-implementation-dependent
([Rust flags](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/fonts/atlas/loader.rs#L44-L100),
[upstream field](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L3939-L3943)).
Custom loaders remain an explicitly unsafe native extension boundary. The safe API exposes built-in
loaders, while `FontLoader::from_raw` documents the callback lifetime, ABI, userdata, and no-unwind
obligations. A Rust-owned safe callback adapter is intentionally outside this refactor while upstream
continues to classify `ImFontLoader` as internal and evolving.

## Raw font input and glyph-range boundaries

The pinned stb font source initializer receives only `FontData`, not `FontDataSize`, when it calls
`stbtt_GetFontOffsetForIndex` and `stbtt_InitFont`. A malformed or truncated slice can therefore make
the native parser read beyond the Rust allocation
([stb initializer](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L4681-L4700)).
The compressed path is stricter still: `stb_decompress_length` unconditionally reads bytes 8 through
11, `stb_decompress` ignores its length argument and scans for an in-band terminator, and the Base85
decoder reads five input bytes per group
([compressed header](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L6229-L6232),
[unbounded decompressor](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L6307-L6331),
[Base85 decoder](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3138-L3151)).
Minimum-length checks can improve diagnostics but cannot make arbitrary input safe.

`AddFontFromMemoryTTF` also stores the caller's `GlyphRanges` pointer in the copied source config;
unlike `GlyphExcludeRanges`, the main `AddFont` path does not duplicate it
([memory source](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3255-L3267),
[config copy](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3087-L3105)).
Raw borrowed sentinel arrays therefore cannot back a safe Rust method. Inclusive `(start, end)`
pairs are validated, encoded, and stored by atlas identity until the native sources are cleared or
the atlas is destroyed.

Finally, ownership transfers even when source initialization fails. Native rollback destroys an
owned `FontData` buffer before returning null, so a Rust null branch must not free that pointer again
([rollback](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3118-L3129),
[source destruction](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_draw.cpp#L3735-L3748)).
The implemented boundary makes external source construction and direct raw-data entry points
unsafe, copies ordinary memory fonts into native allocations, retains structured ranges, preloads
file sources before batch mutation, and validates merge-mode constraints before native assertions.

## Unknown-count list clipping is a distinct protocol

The remaining deferred list-clipper function cannot be added safely as an unrestricted method:

- Dear ImGui reserves `INT_MAX` as the sentinel for an unknown item count. In that mode the final
  `Step()` does not advance the cursor automatically
  ([public contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.h#L2994-L3023)).
- `SeekCursorForItem()` is documented for one purpose: after unknown-count stepping has completed,
  call it with the discovered final count
  ([implementation contract](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui.cpp#L3391-L3429)).
- The upstream demo uses exactly that sequence for a lazily traversed tree
  ([demo](https://github.com/ocornut/imgui/blob/b61e56346a92cfcaf1f43a545ca37b0b32239654/imgui_demo.cpp#L9816-L9844)).

The current Rust constructor accepts every `usize` that fits `i32`, including `i32::MAX`, and then
records it as an ordinary known count
([Rust conversion and constructor](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/list_clipper.rs#L8-L64)).
That maximum value is therefore silently reinterpreted by native code as unknown-count mode, but the
Rust token exposes neither a final-count operation nor a warning. A safe design needs separate known
and unknown constructors, must reject `i32::MAX` on the known path, and must consume the unknown
token through a finalizer that performs the post-step seek in the only valid state.

That design is implemented as `ListClipper::unknown_count()`, fused `next_range()`, and consuming
`finish(final_items_count)`. The known constructor accepts at most `i32::MAX - 1`; the unknown final
count may equal `i32::MAX`.

## PR #42: actual change and validation state

PR #42 has one commit and changes one line in `dear-imgui/Cargo.toml`: it replaces
`default = ["tracing"]` with `default = []`
([commit](https://github.com/Latias94/dear-imgui-rs/pull/42/commits/48f559933731e02de0f9cfea4e1f3b52c83c7a51),
[patched manifest](https://github.com/DBLouis/dear-imgui-rs/blob/48f559933731e02de0f9cfea4e1f3b52c83c7a51/dear-imgui/Cargo.toml#L18-L35)).
Both `tracing` and `tracing-subscriber` are already optional dependencies on `main`; the current
default feature merely enables them
([base manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/Cargo.toml#L18-L35)).

Therefore the PR body statement that tracing is still built with `default-features = false` is not
true for a direct core dependency. The following local graph checks on the pinned repository show
no tracing packages in the latter graph:

```text
cargo tree -p dear-imgui-rs -e features --depth 2
cargo tree -p dear-imgui-rs -e features --depth 2 --no-default-features
```

The default graph contained 33 normal packages and the no-default graph 17 on the audited Windows
host. Most of the 16-package difference comes from `tracing-subscriber` and its filtering/formatting
dependencies, rather than from `tracing` alone.

The author is a first-time contributor. The main CI run is waiting for approval and has zero jobs
([CI run](https://github.com/Latias94/dear-imgui-rs/actions/runs/29474716507)); the only completed PR
check is the labeler
([label job](https://github.com/Latias94/dear-imgui-rs/actions/runs/29474714771/job/87545118647)).
At research time the PR was mergeable but had no review, discussion, or substantive CI result.

The implemented branch keeps the contributor's opt-in direction but supersedes the one-line patch:
core logging policy is removed rather than left behind a disabled fallback, WGPU events become
independently optional, examples own subscriber setup, and feature graphs are verified explicitly.

### Feature propagation through repository crates

Cargo unifies enabled features for a package, and default features are enabled unless every
relevant dependency edge opts out
([Cargo feature unification](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification),
[default-feature caveat](https://doc.rust-lang.org/cargo/reference/features.html#the-default-feature)).
The workspace dependency for `dear-imgui-rs` does not disable defaults
([workspace manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/Cargo.toml#L68-L76)),
and backends/extensions consume that workspace dependency normally. For example, winit does so even
when its own default features are disabled
([winit manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/backends/dear-imgui-winit/Cargo.toml#L14-L32)).

Local `cargo tree -e features --no-default-features` checks found the core default/tracing edge was
still enabled through each of the audited `dear-imgui-winit`, `dear-imgui-wgpu`,
`dear-imgui-glow`, `dear-imgui-sdl3`, `dear-imgui-ash`, `dear-imgui-bevy`, `dear-app`,
`dear-implot`, and `dear-imgui-reflect` packages. PR #42 removes that particular propagation by
making the core default set empty. It does not remove independent tracing dependencies: WGPU and
`dear-app`, for example, directly depend on tracing
([WGPU manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/backends/dear-imgui-wgpu/Cargo.toml#L14-L28),
[`dear-app` manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-app/Cargo.toml#L14-L31)).

The implementation also sets `default-features = false` on the workspace core dependency so future
core defaults cannot leak through internal edges. WGPU now gates its direct tracing dependency; the
application-level `dear-app` dependency remains intentional.

### User-visible behavior and logging ownership risks

This default change is source-compatible for ordinary core APIs but is not behavior-neutral:

- With tracing enabled, three helpers install a process-global formatted subscriber. With it
  disabled, the same names only print a warning to stderr; the remaining helper functions are
  no-ops
  ([enabled and fallback paths](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/logging.rs#L9-L136)).
- `SubscriberBuilder::init()` panics if another global subscriber is already installed
  ([tracing-subscriber 0.3.23 source](https://github.com/tokio-rs/tracing/blob/54ede4d5d85a536aed5485c5213011d9ec961935/tracing-subscriber/src/fmt/mod.rs#L510-L521)).
- Seven `ImGuiError` constructors emit events when tracing is enabled and become silent when it is
  disabled
  ([constructors](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/error.rs#L58-L120)).
- Native examples call the core initialization helpers and independently depend on tracing crates,
  but do not explicitly enable `dear-imgui-rs/tracing`. After PR #42, those calls select the warning
  fallback unless their core feature edge is updated
  ([examples manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/examples/Cargo.toml#L292-L315)).
- The module documentation claims both tracing and `log` backends, but there is no `log` dependency
  or implementation in the module
  ([module header](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/logging.rs#L1-L7),
  [core dependencies](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/Cargo.toml#L18-L28)).
- Exported `imgui_*` macros place `#[cfg(feature = "tracing")]` and a bare `tracing::` path in the
  expansion. That feature test is evaluated in the downstream crate's feature namespace, and the
  path requires the downstream crate to resolve `tracing`; the existing test only expands the
  macros inside the defining crate
  ([macros and test](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/logging.rs#L138-L193),
  [Cargo feature scope](https://doc.rust-lang.org/cargo/reference/features.html#the-features-section)).

The selected boundary is that core owns neither event emission nor process-global subscriber
policy. Errors remain values without logging side effects. Backends may expose opt-in diagnostic
events, and final applications choose and install subscribers.

## Baseline direct core dependency classification

The following classification describes the current implementation and public-type exposure. It is
not a final removal plan.

| Dependency | Baseline role and public exposure | Baseline classification / question |
| --- | --- | --- |
| `dear-imgui-sys` | Re-exported as `sys` and used in public signatures and conversion bounds ([re-export](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/lib.rs#L267-L274)). | Core contract dependency. |
| `bitflags` | Generates many public flag value types across core APIs ([example](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/io/flags.rs#L1-L130)). | Core implementation dependency. |
| `parking_lot` | Supplies the crate-private reentrant mutex guarding current-context binding ([binding](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/context/binding.rs#L1-L20)). No public `parking_lot` type leaks. | Required by the current reentrant binding architecture; replacement cost is architectural, not feature gating. |
| `thiserror` | Derives `Error` for always-available public error enums; no `thiserror` type appears in their public fields ([core error](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/error.rs#L4-L49), [snapshot error](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/render/snapshot.rs#L8-L78)). | Required by current implementations, but not a public type contract. |
| `serde` | Adds feature-gated trait implementations/derives to existing public values ([example](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/input/keyboard.rs#L1-L12)). | Correctly optional. |
| `glam` | The core feature forwards `dear-imgui-sys/glam`, while actual `From<glam::Vec*>` implementations live in sys. Core source uses `glam` only in crate documentation ([feature](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/Cargo.toml#L47-L50), [sys conversions](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/src/lib.rs#L985-L1060)). | Optional interop is valid; the final core feature only forwards sys support and keeps `glam` as a dev dependency for documentation/tests. |
| `mint` | Core production source does not use a `mint` item; crate documentation demonstrates it. The conversions are implemented by the required sys crate ([core docs](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/lib.rs#L30-L53), [sys conversions](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/src/lib.rs#L985-L1060)). | No observed production need for the direct core edge; doctest/dev placement is an open cleanup question. |
| `cfg-if` | Declared by core and sys, but repository-wide source search found no `cfg_if!` or `cfg_if` use ([core manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/Cargo.toml#L18-L28), [sys manifest](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/Cargo.toml#L39-L48)). | No observed code role; removal is a dependency-cleanup question. |
| `tracing` | Used only by `logging.rs` and `error.rs`; no tracing type appears in ordinary public signatures ([uses](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/logging.rs#L6-L103), [error uses](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/error.rs#L6-L10)). | Optional diagnostics dependency; exported macros currently leak assumptions about downstream features and paths. |
| `tracing-subscriber` | Used only by the three initialization helpers that choose a global filter/format ([helpers](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/src/logging.rs#L9-L64)). | Application-policy/helper-crate pressure rather than core rendering functionality. |

The final dependency result removes core `tracing`, `tracing-subscriber`, and unused `cfg-if` edges;
moves direct core `mint`/`glam` use to dev dependencies while retaining the `glam` forwarding
feature; removes unused `cfg-if` edges from extension sys crates, Ash's unused `log`, and Glow's
`memoffset` in favor of `std::mem::offset_of!`.

## Native prebuilt implications

PR #42 cannot change a native prebuilt. Its feature does not forward to `dear-imgui-sys`, and the
prebuilt consumer explicitly sets `dear-imgui-rs` `default-features = false`
([core feature graph](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui/Cargo.toml#L30-L50),
[prebuilt consumer](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/tools/ci/verify_packaged_core.sh#L36-L63)).
It changes neither C++ source selection nor ABI, archive contents, native link metadata, or artifact
identity.

The implemented native `prebuilt` dependency split is:

```text
dear-imgui-sys target-normal graph: mint
dear-imgui-sys host build graph:     build-support[download] -> ureq + flate2 + tar
package helper target graph:         flate2 + tar (artifact metadata comes from build.rs OUT_DIR)
```

This keeps HTTP and extraction code out of downstream target libraries, rejects `wasm + prebuilt`
before dependency selection can masquerade as a supported provider, and avoids reintroducing
`build-support` as a normal dependency of the packaging executable.

### Baseline native matrix and identity

The workflow has five target/CRT entries and builds four core profiles for each: normal,
FreeType, stack layout, and stack layout plus FreeType. That is 20 core archives per complete run
([target matrix](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/.github/workflows/prebuilt-binaries.yml#L37-L63),
[profile builds](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/.github/workflows/prebuilt-binaries.yml#L165-L246)).
The verifier consumes all four and checks normal/stack-layout profile mismatches fail
([profile map](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/tools/ci/verify_packaged_core.sh#L36-L63),
[round trips](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/tools/ci/verify_packaged_core.sh#L231-L295)).

The artifact manifest/identity includes crate version, target, link/CRT mode, normalized native
features, cimgui and ImGui revisions, and binding specification hash, and comparison is strict
([identity model](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/tools/build-support/src/lib.rs#L967-L1057),
[release documentation](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/docs/RELEASING.md#L99-L108)).

### Baseline feature classification

| Feature | Native artifact effect | Matrix/identity status |
| --- | --- | --- |
| `freetype` | Defines FreeType/STB configuration, compiles `imgui_freetype.cpp`, adds headers and external native link metadata ([build](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L581-L594), [prebuilt relink metadata](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L1000-L1009)). | Correctly present as both FreeType profiles and in artifact identity. The identity records the feature, not the discovered FreeType version/link form. |
| `stack-layout` | Rewrites the copied `imgui.cpp`, adds a native shim, and exposes feature-gated Rust ABI ([native patch](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L559-L565), [shim build](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L599-L721), [Rust ABI and WASM rejection](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/src/lib.rs#L88-L342)). | Correctly present with/without FreeType and rejected for WASM. |
| `test-engine` | Defines `IMGUI_ENABLE_TEST_ENGINE` and compiles hook source, so it changes native code and symbols ([build](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L573-L580), [hooks](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/src/imgui_test_engine_hooks.cpp#L1-L73)). | Included in artifact feature/name/profile construction, but `build.rs` forces source build whenever it is enabled and the official workflow/verifier has no test-engine profile ([artifact feature](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L75-L86), [force-source branch](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L261-L275), [artifact suffix/profile](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L1040-L1054)). This is a half-supported prebuilt contract. |
| `wasm` | Selects import-style pregenerated bindings and skips native core/prebuilt selection on supported WASM targets ([binding selection](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L101-L109), [native bypass](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L261-L288)). | Correctly absent from the native archive matrix. At baseline, `wasm,test-engine` incorrectly checked successfully without native hooks; the final contract rejects that combination. |
| `glam`, `bindgen`, `package-bin`, `prebuilt`, `build-from-source`, `pkg-config`, `vcpkg` | Rust interop, binding generation, packaging/selection, or dependency-discovery policy. `pkg-config`/`vcpkg` affect linking only through the FreeType artifact profile. | Correctly absent as independent core artifact identities. |
| `backend-shim-*` | Compile separate backend native libraries, not the packaged `dear_imgui` core archive ([shim selection](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L726-L908)). | Correctly absent from core archive identity; `IMGUI_SYS_SKIP_CC` explicitly rejects them ([contract](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/dear-imgui-sys/build.rs#L233-L242)). |

Two baseline verification gaps were found:

1. The packaged-core selector ignores an archive whose target/CRT matches but whose feature set is
   not one of the four known profiles, rather than rejecting the unknown profile
   ([selector](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/tools/ci/verify_packaged_core.sh#L114-L156)).
2. Once PR #42 empties defaults, the ordinary release tests no longer exercise the enabled tracing
   branch. The explicit feature profiles currently cover multi-viewport and stack-layout, not
   tracing
   ([pre-publish profiles](https://github.com/Latias94/dear-imgui-rs/blob/3a748122d9c557b6229f258d2e2b9bacc4178e4a/tools/pre_publish_check.py#L151-L225)).

Both are resolved in the implementation. The selector rejects a target/CRT-matching archive whose
features do not map to a supported profile, and WGPU is checked with tracing both disabled and
enabled. Core forwards `prebuilt` and `build-from-source`; Cargo feature unification is supported by
giving source building precedence. `test-engine` now enables `build-from-source` in the sys crate and
is rejected with the WASM provider.

## Final design decisions

1. `FontAtlas` is a non-owning transparent view borrowed directly from `Context`; standalone
   ownership belongs only to `SharedFontAtlas`. This uses Rust borrow lifetimes rather than a public
   runtime lease abstraction. Renderer capability coordination resets after the last context
   unregisters, but switching a legacy-built atlas to managed textures requires clearing and
   repopulating it first, while switching from managed textures to a legacy renderer requires a full
   legacy `build()`.
2. Baked access supports the current font and an arbitrary validated `FontId`, size, and density,
   but only through `Ui`. The baked view is frame-bound; every glyph result is an owned `Glyph`
   metrics copy, and the safe API exposes no UVs that same-frame repacking could invalidate.
3. Safe borrowed atlas texture escape hatches are removed. Renderers inspect `TextureData` through
   the `FontAtlasTexture<'_>` returned by `tex_data()` and use the narrow
   `texture_id`/unsafe `set_texture_id` legacy feedback path. Raw pointers remain explicit FFI
   values and do not bypass the lease through a safe Rust reference. The lease blocks invalidating
   operations and frame advancement.
4. External font bytes and files are an unsafe native-parser boundary. `FontSource` is opaque,
   memory-font bytes are copied, file batches are preloaded, and direct glyph ranges are structured
   pairs retained by atlas identity.
5. Custom-rectangle allocation includes the first complete pixel write. Later complete replacements
   are supported and queue the current exact texture region only for managed-texture renderers;
   mutation after a completed legacy upload is rejected before changing atlas state.
6. `CustomRectId` survives repacking but not removal or builder clear. `CustomRectSnapshot` is a
   short-lived copy-out view and is resolved again for each draw submission.
7. Safe custom Rust font-loader callbacks are not introduced while upstream keeps the interface
   internal. Built-in loaders remain safe and external callback tables remain an unsafe contract.
8. Core owns neither tracing events nor subscriber setup. WGPU exposes optional events; native
   examples and final applications own subscriber policy.
9. Repository-internal core edges disable defaults explicitly, even though the current default set
   is empty.
10. `prebuilt` is native-only. Download/extraction dependencies remain in the host build graph, and
   the package executable consumes build-script-generated metadata without a normal build-support
   dependency.
11. `test-engine` is source-only, is not a prebuilt profile dimension, and is invalid with WASM.
    `prebuilt` and `build-from-source` may be unified, with source building taking precedence.
12. Dependencies without a production role were removed or moved to dev scope; math conversion
   ownership remains in sys and the high-level `glam` feature only forwards that capability.
13. Known and unknown list clippers share one context registry that enforces native LIFO and the
    exact frame/window-Begin/table-instance scope. Out-of-order drops are abandoned until they become
    stack-top; wrong-scope cleanup disables cursor seeking but delegates native stack restoration to
    `End()`, and unknown completion performs the final seek without draining frozen table rows.

## Reproducible read-only checks

The following commands were used against the baseline:

```text
git submodule status
gh pr view 42 --repo Latias94/dear-imgui-rs --json ...
gh pr diff 42 --repo Latias94/dear-imgui-rs
cargo tree -p dear-imgui-rs -e features --depth 2
cargo tree -p dear-imgui-rs -e features --depth 2 --no-default-features
cargo check -p dear-imgui-rs --lib
cargo check -p dear-imgui-rs --lib --no-default-features
cargo nextest run -p dear-imgui-rs --no-default-features
cargo check -p dear-imgui-sys --target wasm32-unknown-unknown \
  --no-default-features --features wasm,test-engine
rg -n "cfg_if!|cfg_if" .
```

The no-default core nextest run passed 283/283 tests. The `wasm,test-engine` sys check also passed,
which is evidence of the feature-contract ambiguity described above, not evidence that native test
engine hooks exist in the WASM provider.
