//! Borrowed access to the two core-owned draw representations.

use dear_imgui_rs::render::{
    DrawCmd, DrawCmdIterator, DrawCmdSnapshot, DrawData, DrawDataSnapshot, DrawIdx, DrawList,
    DrawListIterator, DrawListSnapshot, DrawVert, RawCallbackCommand, TextureBinding,
};

#[derive(Clone, Copy)]
pub(super) enum DrawSource<'a> {
    Live(&'a DrawData),
    Snapshot(&'a DrawDataSnapshot),
}

impl<'a> DrawSource<'a> {
    pub(super) fn display_pos(self) -> [f32; 2] {
        match self {
            Self::Live(draw) => draw.display_pos(),
            Self::Snapshot(draw) => draw.display_pos,
        }
    }

    pub(super) fn display_size(self) -> [f32; 2] {
        match self {
            Self::Live(draw) => draw.display_size(),
            Self::Snapshot(draw) => draw.display_size,
        }
    }

    pub(super) fn framebuffer_scale(self) -> [f32; 2] {
        match self {
            Self::Live(draw) => draw.framebuffer_scale(),
            Self::Snapshot(draw) => draw.framebuffer_scale,
        }
    }

    pub(super) fn lists(self) -> DrawLists<'a> {
        match self {
            Self::Live(draw) => DrawLists::Live(draw.draw_lists()),
            Self::Snapshot(draw) => DrawLists::Snapshot(draw.draw_lists.iter()),
        }
    }
}

pub(super) enum DrawLists<'a> {
    Live(DrawListIterator<'a>),
    Snapshot(std::slice::Iter<'a, DrawListSnapshot>),
}

impl<'a> Iterator for DrawLists<'a> {
    type Item = DrawListSource<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Live(lists) => lists.next().map(DrawListSource::Live),
            Self::Snapshot(lists) => lists.next().map(DrawListSource::Snapshot),
        }
    }
}

pub(super) enum DrawListSource<'a> {
    Live(&'a DrawList),
    Snapshot(&'a DrawListSnapshot),
}

impl<'a> DrawListSource<'a> {
    pub(super) fn vertices(&self) -> &'a [DrawVert] {
        match self {
            Self::Live(list) => list.vtx_buffer(),
            Self::Snapshot(list) => &list.vtx,
        }
    }

    pub(super) fn indices(&self) -> &'a [DrawIdx] {
        match self {
            Self::Live(list) => list.idx_buffer(),
            Self::Snapshot(list) => &list.idx,
        }
    }

    pub(super) fn commands(&self) -> DrawCommands<'a> {
        match self {
            Self::Live(list) => DrawCommands::Live(list.commands()),
            Self::Snapshot(list) => DrawCommands::Snapshot(list.commands.iter()),
        }
    }
}

pub(super) enum DrawCommand<'a> {
    Elements {
        count: usize,
        clip_rect: [f32; 4],
        texture: TextureBinding,
        vtx_offset: usize,
        idx_offset: usize,
    },
    ResetRenderState,
    SetSamplerLinear,
    SetSamplerNearest,
    SetSamplerFromTexture,
    RawCallback(RawCallbackCommand<'a>),
}

pub(super) enum DrawCommands<'a> {
    Live(DrawCmdIterator<'a>),
    Snapshot(std::slice::Iter<'a, DrawCmdSnapshot>),
}

impl<'a> Iterator for DrawCommands<'a> {
    type Item = DrawCommand<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self {
            Self::Live(commands) => match commands.next()? {
                DrawCmd::Elements { count, cmd_params } => DrawCommand::Elements {
                    count,
                    clip_rect: cmd_params.clip_rect,
                    texture: TextureBinding::Legacy(cmd_params.texture_id),
                    vtx_offset: cmd_params.vtx_offset,
                    idx_offset: cmd_params.idx_offset,
                },
                DrawCmd::ResetRenderState => DrawCommand::ResetRenderState,
                DrawCmd::SetSamplerLinear => DrawCommand::SetSamplerLinear,
                DrawCmd::SetSamplerNearest => DrawCommand::SetSamplerNearest,
                DrawCmd::SetSamplerFromTexture => DrawCommand::SetSamplerFromTexture,
                DrawCmd::RawCallback(callback) => DrawCommand::RawCallback(callback),
            },
            Self::Snapshot(commands) => match *commands.next()? {
                DrawCmdSnapshot::Elements {
                    count,
                    clip_rect,
                    texture,
                    vtx_offset,
                    idx_offset,
                } => DrawCommand::Elements {
                    count,
                    clip_rect,
                    texture,
                    vtx_offset,
                    idx_offset,
                },
                DrawCmdSnapshot::ResetRenderState => DrawCommand::ResetRenderState,
                DrawCmdSnapshot::SetSamplerLinear => DrawCommand::SetSamplerLinear,
                DrawCmdSnapshot::SetSamplerNearest => DrawCommand::SetSamplerNearest,
                DrawCmdSnapshot::SetSamplerFromTexture => DrawCommand::SetSamplerFromTexture,
            },
        })
    }
}
