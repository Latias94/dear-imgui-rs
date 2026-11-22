#include "backends/imgui_impl_sdlrenderer3.cpp"

// The upstream updater acknowledges partial uploads even when SDL_UpdateTexture fails.
// Leave a failed request pending so the Rust store can roll back its pixels and retry.
static void update_texture(ImTextureData* tex, decltype(&SDL_UpdateTexture) upload) {
    if (tex->Status == ImTextureStatus_WantCreate) {
        SDL_Texture* texture = SDL_CreateTexture(ImGui_ImplSDLRenderer3_GetBackendData()->Renderer,
            SDL_PIXELFORMAT_RGBA32, SDL_TEXTUREACCESS_STATIC, tex->Width, tex->Height);
        if (!texture)
            return;
        if (!upload(texture, nullptr, tex->GetPixels(), tex->GetPitch())
            || !SDL_SetTextureBlendMode(texture, SDL_BLENDMODE_BLEND)
            || !SDL_SetTextureScaleMode(texture, SDL_SCALEMODE_LINEAR)) {
            SDL_DestroyTexture(texture);
            return;
        }
        tex->SetTexID((ImTextureID)(intptr_t)texture);
    } else if (tex->Status == ImTextureStatus_WantUpdates) {
        SDL_Texture* texture = (SDL_Texture*)(intptr_t)tex->GetTexID();
        for (const ImTextureRect& r : tex->Updates) {
            const SDL_Rect rect = { r.x, r.y, r.w, r.h };
            if (!upload(texture, &rect, tex->GetPixelsAt(r.x, r.y), tex->GetPitch()))
                return;
        }
    } else {
        ImGui_ImplSDLRenderer3_UpdateTexture(tex);
        return;
    }
    tex->SetStatus(ImTextureStatus_OK);
}

extern "C" void dear_imgui_sdl3_backend_sdlrenderer3_update_texture(ImTextureData* tex) {
    update_texture(tex, SDL_UpdateTexture);
}

#if defined(DEAR_IMGUI_SDL3_NATIVE_SELF_TEST)
extern "C" bool dear_imgui_sdl3_backend_sdlrenderer3_texture_self_test() {
    ImTextureData tex;
    tex.Create(ImTextureFormat_RGBA32, 4, 2);
    tex.Updates.push_back({0, 0, 1, 1});
    tex.Updates.push_back({2, 1, 1, 1});
    tex.SetStatus(ImTextureStatus_WantUpdates);
    update_texture(&tex, [](SDL_Texture*, const SDL_Rect* rect, const void*, int) { return rect->x == 0; });
    if (tex.Status != ImTextureStatus_WantUpdates)
        return false;
    update_texture(&tex, [](SDL_Texture*, const SDL_Rect*, const void*, int pitch) { return pitch == 16; });
    return tex.Status == ImTextureStatus_OK;
}
#endif
