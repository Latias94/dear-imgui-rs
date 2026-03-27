#define SDL_MAIN_HANDLED
#import <SDL3/SDL.h>
#import <SDL3/SDL_main.h>
#import "dear_imgui_ios_sdl3_smoke.h"

int main(int argc, char *argv[]) {
    return SDL_RunApp(argc, argv, dear_imgui_ios_sdl3_smoke_main, NULL);
}
