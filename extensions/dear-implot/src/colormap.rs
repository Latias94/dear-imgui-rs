use crate::{ColormapIndex, sys};

/// Built-in colormaps
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Colormap {
    Deep = sys::ImPlotColormap_Deep as i32,
    Dark = sys::ImPlotColormap_Dark as i32,
    Pastel = sys::ImPlotColormap_Pastel as i32,
    Paired = sys::ImPlotColormap_Paired as i32,
    Viridis = sys::ImPlotColormap_Viridis as i32,
    Plasma = sys::ImPlotColormap_Plasma as i32,
    Hot = sys::ImPlotColormap_Hot as i32,
    Cool = sys::ImPlotColormap_Cool as i32,
    Pink = sys::ImPlotColormap_Pink as i32,
    Jet = sys::ImPlotColormap_Jet as i32,
    Twilight = sys::ImPlotColormap_Twilight as i32,
    RdBu = sys::ImPlotColormap_RdBu as i32,
    BrBG = sys::ImPlotColormap_BrBG as i32,
    PiYG = sys::ImPlotColormap_PiYG as i32,
    Spectral = sys::ImPlotColormap_Spectral as i32,
    Greys = sys::ImPlotColormap_Greys as i32,
}

impl Colormap {
    #[inline]
    pub const fn index(self) -> ColormapIndex {
        match ColormapIndex::from_raw(self as i32) {
            Some(index) => index,
            None => panic!("built-in ImPlot colormap index must be valid"),
        }
    }
}
