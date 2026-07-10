use std::ptr;

use dear_imgui_rs::{Context, TextureData, TextureId, TextureStatus};

pub(crate) fn reset_for_new_gpu_generation(context: &mut Context) -> usize {
    let mut reset_count = 0;
    let mut textures = context.platform_io_mut().textures_mut();
    while let Some(mut texture) = textures.next() {
        reset_texture(&mut texture);
        reset_count += 1;
    }
    reset_count
}

fn reset_texture(texture: &mut TextureData) {
    let next_status = match texture.status() {
        TextureStatus::OK | TextureStatus::WantCreate | TextureStatus::WantUpdates => {
            TextureStatus::WantCreate
        }
        TextureStatus::WantDestroy | TextureStatus::Destroyed => TextureStatus::Destroyed,
    };
    if next_status == TextureStatus::Destroyed {
        texture.destroy_pixels();
        texture.set_status(TextureStatus::Destroyed);
    } else {
        texture.set_backend_user_data(ptr::null_mut());
        texture.set_tex_id(TextureId::null());
        texture.set_status(TextureStatus::WantCreate);
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;

    use dear_imgui_rs::{TextureData, TextureId, TextureStatus};

    use super::reset_texture;

    fn texture_with_status(status: TextureStatus) -> dear_imgui_rs::OwnedTextureData {
        let mut texture = TextureData::new();
        if status != TextureStatus::Destroyed {
            texture.create(dear_imgui_rs::TextureFormat::RGBA32, 1, 1);
        }
        texture.set_tex_id(TextureId::new(99));
        texture.set_backend_user_data(std::ptr::dangling_mut::<c_void>());
        texture.set_status(status);
        texture
    }

    #[test]
    fn live_and_pending_managed_textures_are_recreated() {
        for status in [
            TextureStatus::OK,
            TextureStatus::WantCreate,
            TextureStatus::WantUpdates,
        ] {
            let mut texture = texture_with_status(status);
            reset_texture(&mut texture);

            assert_eq!(texture.status(), TextureStatus::WantCreate);
            assert!(texture.tex_id().is_null());
            assert!(texture.backend_user_data().is_null());
        }
    }

    #[test]
    fn destroyed_managed_textures_remain_destroyed() {
        for status in [TextureStatus::WantDestroy, TextureStatus::Destroyed] {
            let mut texture = texture_with_status(status);
            reset_texture(&mut texture);

            assert_eq!(texture.status(), TextureStatus::Destroyed);
            assert!(texture.tex_id().is_null());
            assert!(texture.backend_user_data().is_null());
        }
    }
}
